"""vaultli package exports."""

from .core import (
    INDEX_FILENAME,
    VAULT_MARKER,
    VaultliError,
    add_file,
    build_index,
    find_root,
    infer_frontmatter,
    init_vault,
    is_sidecar_markdown,
    load_index_records,
    make_id,
    parse_markdown_file,
    scaffold_file,
    search_index,
    show_record,
    validate_vault,
)
from .assemble import assemble_context
from .federation import federated_load, federated_search, resolve_federated_id
from .git import apply_git_meta, git_meta_for_vault, git_meta_for_file, GitMeta

__all__ = [
    "INDEX_FILENAME",
    "VAULT_MARKER",
    "VaultliError",
    "add_file",
    "build_index",
    "find_root",
    "infer_frontmatter",
    "init_vault",
    "is_sidecar_markdown",
    "load_index_records",
    "make_id",
    "parse_markdown_file",
    "scaffold_file",
    "search_index",
    "show_record",
    "validate_vault",
    # Context assembly
    "assemble_context",
    # Federation
    "federated_load",
    "federated_search",
    "resolve_federated_id",
    # Git integration
    "apply_git_meta",
    "git_meta_for_vault",
    "git_meta_for_file",
    "GitMeta",
]
