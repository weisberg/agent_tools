#!/usr/bin/env node

import { readFile } from "node:fs/promises"
import process from "node:process"

const text = await readStdin()

try {
  const request = JSON.parse(text || "{}")
  const result = process.env.FRAMERLI_BRIDGE_MOCK === "1"
    ? await runMock(request)
    : await runLive(request)
  write({ ok: true, data: result })
} catch (error) {
  write({
    ok: false,
    error: normalizeError(error),
  })
  process.exitCode = exitFor(error)
}

async function readStdin() {
  let data = ""
  process.stdin.setEncoding("utf8")
  for await (const chunk of process.stdin) data += chunk
  return data
}

function write(value) {
  process.stdout.write(`${JSON.stringify(value)}\n`)
}

function normalizeError(error) {
  return {
    code: error.code || "E_BRIDGE",
    message: error.message || String(error),
    hint: error.hint,
    retryable: Boolean(error.retryable),
    details: error.details,
  }
}

function exitFor(error) {
  switch (error.code) {
    case "E_USAGE":
      return 2
    case "E_AUTH_MISSING":
    case "E_AUTH_INVALID":
      return 3
    case "E_NOT_FOUND":
      return 4
    case "E_APPROVAL_REQUIRED":
    case "E_CONFLICT":
      return 5
    case "E_RATE_LIMITED":
      return 6
    case "E_COLD_START_TIMEOUT":
      return 7
    case "E_NETWORK":
      return 8
    default:
      return 10
  }
}

async function runLive(request) {
  const project = request.project || process.env.FRAMERLI_PROJECT
  const apiKey = process.env.FRAMER_API_KEY
  if (!project) {
    throw coded("E_USAGE", "No Framer project URL configured.", {
      hint: "Pass --project, set FRAMERLI_PROJECT, or configure profile.<name>.project in framerli.toml.",
    })
  }
  if (!apiKey) {
    throw coded("E_AUTH_MISSING", "No Framer API key configured.", {
      hint: "Set FRAMER_API_KEY. Persistent keychain storage is intentionally left to the Rust CLI/profile layer.",
    })
  }

  let sdk
  try {
    sdk = await import("framer-api")
  } catch (error) {
    throw coded("E_BRIDGE_DEPENDENCY", "The framer-api package is not installed for the bridge.", {
      hint: "Run npm install in tools/framerli/bridge, or point FRAMERLI_BRIDGE at a packaged bridge with framer-api available.",
      details: { cause: error.message },
    })
  }

  const connect = sdk.connect || sdk.framer?.connect || sdk.default?.connect
  if (typeof connect !== "function") {
    throw coded("E_BRIDGE_DEPENDENCY", "Could not find connect() in the framer-api package.", {
      hint: "The bridge may need a small adapter update for the installed framer-api version.",
      details: { exports: Object.keys(sdk) },
    })
  }

  const client = await connect(project, apiKey)
  try {
    return await dispatch(client, request)
  } finally {
    if (typeof client?.disconnect === "function") {
      await client.disconnect()
    } else if (typeof client?.[Symbol.asyncDispose] === "function") {
      await client[Symbol.asyncDispose]()
    }
  }
}

async function dispatch(client, request) {
  const op = request.operation
  const args = request.args || {}
  switch (op) {
    case "project.info":
      return {
        project: await optionalCall(client, "getProjectInfo"),
        publish: await optionalCall(client, "getPublishInfo"),
      }
    case "whoami":
      return await call(client, "getCurrentUser")
    case "can":
      return await call(client, "isAllowedTo", args.method)
    case "status":
      return await call(client, "getChangedPaths")
    case "contributors":
      return await call(client, "getChangeContributors", args.from, args.to)
    case "publish": {
      const deployment = await call(client, "publish")
      if (args.promote) {
        const deploymentId = deployment?.id || deployment?.deploymentId || deployment
        const promoted = await call(client, "deploy", deploymentId)
        return { deployment, promoted }
      }
      return deployment
    }
    case "deploy":
      return await call(client, "deploy", args.deploymentId)
    case "cms.collections.list":
      return await listCollections(client)
    case "cms.collection.show":
      return await showCollection(client, args.collection)
    case "cms.fields.list": {
      const collection = await findCollection(client, args.collection)
      return await fieldsFor(collection)
    }
    case "cms.items.list": {
      const collection = await findCollection(client, args.collection)
      return await call(collection, "getItems", { limit: args.limit, cursor: args.cursor, where: args.where })
    }
    case "cms.items.get": {
      const collection = await findCollection(client, args.collection)
      const items = await call(collection, "getItems")
      const found = arrayify(items).find((item) => {
        return item?.id === args.idOrSlug || item?.slug === args.idOrSlug || item?.fieldData?.slug === args.idOrSlug
      })
      if (!found) throw coded("E_NOT_FOUND", `CMS item '${args.idOrSlug}' was not found.`, { details: { collection: args.collection } })
      return found
    }
    case "cms.items.add": {
      const collection = await findCollection(client, args.collection)
      const items = await readItems(args.file)
      return await call(collection, "addItems", items)
    }
    case "cms.items.remove": {
      const collection = await findCollection(client, args.collection)
      return await call(collection, "removeItems", args.ids)
    }
    case "introspect":
      return {
        project: await optionalCall(client, "getProjectInfo"),
        collections: await listCollections(client),
        changedPaths: await optionalCall(client, "getChangedPaths"),
      }
    default:
      throw coded("E_NOT_IMPLEMENTED", `Bridge operation '${op}' is not implemented yet.`, {
        hint: "The Rust CLI accepts the command, but this bridge currently covers the core project/CMS/publish milestone.",
        details: { operation: op },
      })
  }
}

async function listCollections(client) {
  const collections = await optionalCall(client, "getCollections")
  const managedCollections = await optionalCall(client, "getManagedCollections")
  return {
    collections: arrayify(collections),
    managedCollections: arrayify(managedCollections),
    count: arrayify(collections).length + arrayify(managedCollections).length,
  }
}

async function showCollection(client, name) {
  const collection = await findCollection(client, name)
  return {
    id: collection.id,
    name: collection.name,
    slug: collection.slug,
    fields: await fieldsFor(collection),
  }
}

async function findCollection(client, name) {
  const all = [
    ...arrayify(await optionalCall(client, "getCollections")),
    ...arrayify(await optionalCall(client, "getManagedCollections")),
  ]
  const found = all.find((collection) => {
    return collection?.id === name || collection?.slug === name || collection?.name === name
  })
  if (!found) throw coded("E_NOT_FOUND", `CMS collection '${name}' was not found.`)
  return found
}

async function fieldsFor(collection) {
  if (typeof collection.getFields === "function") return await collection.getFields()
  return collection.fields || collection.fieldDefinitions || []
}

async function readItems(file) {
  if (!file) {
    const input = await readStdin()
    return parseItems(input)
  }
  return parseItems(await readFile(file, "utf8"))
}

function parseItems(input) {
  const trimmed = input.trim()
  if (!trimmed) return []
  if (trimmed.startsWith("[")) return JSON.parse(trimmed)
  return trimmed.split(/\r?\n/).filter(Boolean).map((line) => JSON.parse(line))
}

async function call(target, method, ...args) {
  if (!target || typeof target[method] !== "function") {
    throw coded("E_NOT_IMPLEMENTED", `SDK method '${method}' is unavailable on this object.`, {
      hint: "The installed framer-api version may differ from the PRD mapping.",
    })
  }
  return await target[method](...args)
}

async function optionalCall(target, method, ...args) {
  if (!target || typeof target[method] !== "function") return null
  return await target[method](...args)
}

function arrayify(value) {
  if (!value) return []
  if (Array.isArray(value)) return value
  if (Array.isArray(value.items)) return value.items
  if (Array.isArray(value.collections)) return value.collections
  return [value]
}

function coded(code, message, rest = {}) {
  const error = new Error(message)
  error.code = code
  Object.assign(error, rest)
  return error
}

async function runMock(request) {
  const args = request.args || {}
  switch (request.operation) {
    case "project.info":
      return {
        project: { id: "mock-project", name: "Mock Framer Site", version: 42 },
        publish: { lastPublishedVersion: 41 },
      }
    case "whoami":
      return { id: "mock-user", email: "agent@example.com" }
    case "can":
      return { method: args.method, allowed: true }
    case "status":
      return { added: [], modified: ["/"], removed: [], count: 1 }
    case "cms.collections.list":
      return {
        collections: [{ id: "blog", name: "Blog", slug: "Blog", managed: false }],
        managedCollections: [],
        count: 1,
      }
    case "cms.items.list":
      return {
        items: [{ id: "post-1", slug: "hello-world", fieldData: { title: "Hello World" } }],
        count: 1,
        truncated: false,
        cursor: null,
      }
    case "cms.items.add":
      return { added: 1, updated: 0, collection: args.collection }
    case "publish":
      return { id: "dep_mock", url: "https://mock.framer.website", promoted: Boolean(args.promote) }
    case "deploy":
      return { deploymentId: args.deploymentId, promoted: true }
    case "introspect":
      return {
        project: { id: "mock-project", name: "Mock Framer Site" },
        collections: [{ id: "blog", name: "Blog" }],
        changedPaths: ["/"],
      }
    default:
      return { operation: request.operation, args, mock: true }
  }
}
