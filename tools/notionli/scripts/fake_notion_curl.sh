#!/usr/bin/env sh
set -eu

method="GET"
url=""
data=""

while [ "$#" -gt 0 ]; do
  case "$1" in
    -X)
      shift
      method="${1:-GET}"
      ;;
    --data)
      shift
      data="${1:-}"
      ;;
    -F)
      shift
      data="$data FORM:${1:-}"
      ;;
    http*)
      url="$1"
      ;;
  esac
  shift || true
done

log="${NOTIONLI_FAKE_CURL_LOG:-}"
if [ -n "$log" ]; then
  printf '%s %s %s\n' "$method" "$url" "$data" >> "$log"
fi

page_id="cccccccc-cccc-cccc-cccc-cccccccccccc"
user='{"object":"user","id":"fake-bot-user","type":"bot","bot":{"owner":{"type":"workspace"}}}'
page='{"object":"page","id":"cccccccc-cccc-cccc-cccc-cccccccccccc","url":"https://notion.so/fake-page","properties":{"Name":{"type":"title","title":[{"plain_text":"notionli smoke"}]}}}'

case "$method $url" in
  "GET https://api.notion.com/v1/users/me")
    printf '%s\n200' "$user"
    ;;
  "POST https://api.notion.com/v1/pages")
    printf '%s\n200' "$page"
    ;;
  "POST https://api.notion.com/v1/file_uploads")
    printf '{"object":"file_upload","id":"aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee","status":"pending","upload_url":"https://api.notion.com/v1/file_uploads/aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee/send","filename":"notionli-smoke.txt","content_type":"text/plain","content_length":null}\n200'
    ;;
  POST*\ */file_uploads/*/send)
    printf '{"object":"file_upload","id":"aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee","status":"uploaded","filename":"notionli-smoke.txt","content_type":"text/plain","content_length":20}\n200'
    ;;
  POST*\ */file_uploads/*/complete)
    printf '{"object":"file_upload","id":"aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee","status":"uploaded","filename":"notionli-smoke.txt","content_type":"text/plain","content_length":20,"number_of_parts":{"total":1,"sent":1}}\n200'
    ;;
  GET*\ */pages/*/markdown)
    printf '{"markdown":"# notionli smoke\n\nCreated by notionli fake smoke test.\n\nhttps://example.com/notionli-smoke.txt"}\n200'
    ;;
  GET*\ */pages/*)
    printf '%s\n200' "$page"
    ;;
  PATCH*\ */pages/*)
    case "$data" in
      *in_trash*true*)
        printf '{"object":"page","id":"%s","in_trash":true,"url":"https://notion.so/fake-page","properties":{"Name":{"type":"title","title":[{"plain_text":"notionli smoke trashed"}]}}}\n200' "$page_id"
        ;;
      *)
        printf '%s\n200' "$page"
        ;;
    esac
    ;;
  GET*\ */blocks/*/children*)
    printf '{"object":"list","results":[{"object":"block","id":"bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb","type":"paragraph","paragraph":{"rich_text":[{"plain_text":"Fake smoke content."}]},"has_children":false}],"has_more":false}\n200'
    ;;
  PATCH*\ */blocks/*/children)
    printf '{"object":"list","results":[{"object":"block","id":"bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb","type":"paragraph","paragraph":{"rich_text":[{"plain_text":"Appended."}]},"has_children":false}],"has_more":false}\n200'
    ;;
  "POST https://api.notion.com/v1/comments")
    printf '{"object":"comment","id":"comment_fake","discussion_id":"discussion_fake","rich_text":[{"plain_text":"notionli smoke comment"}]}\n200'
    ;;
  *)
    printf '{"message":"unexpected fake Notion request","method":"%s","url":"%s"}\n500' "$method" "$url"
    ;;
esac
