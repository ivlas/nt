use clap::CommandFactory;
use clap_complete::{Shell as ClapShell, generate};

use crate::cli::{Cli, Shell};

pub fn print_completion(shell: Shell) {
    print!("{}", completion_script(shell));
}

fn completion_script(shell: Shell) -> String {
    let clap_shell = match shell {
        Shell::Bash => ClapShell::Bash,
        Shell::Zsh => ClapShell::Zsh,
    };

    let mut command = Cli::command();
    let mut bytes = Vec::new();
    generate(clap_shell, &mut command, "nt", &mut bytes);
    let mut script = String::from_utf8(bytes).unwrap_or_default();

    match shell {
        Shell::Bash => script.push_str(BASH_NOTE_ID_COMPLETION),
        Shell::Zsh => {
            script = script.replace(":id:_default", ":id:_nt_note_ids");
            script = script.replace(":from_id:_default", ":from_id:_nt_note_ids");
            script = script.replace(":to_id:_default", ":to_id:_nt_note_ids");
            script = script.replace("*::metadata:_default", "*::metadata:_nt_add_metadata");
            script = script.replace("*::expr:_default", "*::expr:_nt_query_expr");
            script = script.replace(":tag:_default", ":tag:_nt_tags");
            script = script.replace("'::name:_default'", "'::name:_nt_vaults'");
            script = script.replace(":name:_default", ":name:_nt_collections");
            script = script.replace(":collection:_default", ":collection:_nt_collections");
            script = script.replace(":kind:_default", ":kind:_nt_kinds");
            script = script.replace(":status:_default", ":status:_nt_statuses");
            script = script.replace(
                "*::args:_default",
                ":id:_nt_note_ids' \\\n':status:_nt_statuses",
            );
            script = script.replace("*::ids:_default", "*::ids:_nt_note_ids");
            insert_zsh_helpers(&mut script);
        }
    }

    script
}

fn insert_zsh_helpers(script: &mut String) {
    const INVOCATION: &str = "\nif [ \"$funcstack[1]\" = \"_nt\" ]; then";

    if let Some(index) = script.find(INVOCATION) {
        script.insert_str(index, ZSH_NOTE_ID_COMPLETION);
    } else {
        script.push_str(ZSH_NOTE_ID_COMPLETION);
    }
}

const BASH_NOTE_ID_COMPLETION: &str = r#"

# nt dynamic completion, backed by visible nt command output.
eval "$(declare -f _nt | sed '1s/^_nt/_nt_clap_complete_generated/')"

_nt_current_token() {
    local line="${COMP_LINE:0:COMP_POINT}"
    local token="${line##* }"
    token="${token##*$'\t'}"
    printf '%s' "$token"
}

_nt_note_ids() {
    local token
    token="$(_nt_current_token)"
    COMPREPLY=( $(compgen -W "$(nt ids 2>/dev/null)" -- "${token}") )
}

_nt_tags() {
    local token
    token="$(_nt_current_token)"
    COMPREPLY=( $(compgen -W "$(nt tags 2>/dev/null | while read -r tag _; do printf '%s\n' "$tag"; done)" -- "${token}") )
}

_nt_collections() {
    local token
    token="$(_nt_current_token)"
    COMPREPLY=( $(compgen -W "$(nt collections 2>/dev/null)" -- "${token}") )
}

_nt_vaults() {
    local token
    token="$(_nt_current_token)"
    COMPREPLY=( $(compgen -W "$(nt config vault 2>/dev/null | while read -r _ name _; do printf '%s\n' "$name"; done)" -- "${token}") )
}

_nt_kinds() {
    local token
    token="$(_nt_current_token)"
    COMPREPLY=( $(compgen -W "note todo meeting decision source research project" -- "${token}") )
}

_nt_statuses() {
    local token
    token="$(_nt_current_token)"
    COMPREPLY=( $(compgen -W "open waiting done dropped" -- "${token}") )
}

_nt_complete_prefixed_values() {
    local token="$1"
    local field="$2"
    shift 2
    local prefix="${field}:"
    local rest list_prefix value_prefix value candidates

    [[ "$token" == "$prefix"* ]] || return 1

    rest="${token#"$prefix"}"
    list_prefix=""
    value_prefix="$rest"
    if [[ "$rest" == *,* ]]; then
        list_prefix="${rest%,*},"
        value_prefix="${rest##*,}"
    fi

    candidates=""
    for value in "$@"; do
        if [[ "$value" == "$value_prefix"* ]]; then
            candidates="${candidates} ${prefix}${list_prefix}${value}"
        fi
    done
    COMPREPLY=( $(compgen -W "$candidates" -- "$token") )
    return 0
}

_nt_tag_values() {
    nt tags 2>/dev/null | while read -r tag _; do printf '%s\n' "$tag"; done
}

_nt_complete_metadata_expr() {
    local token="$1"
    local fields="$2"
    local field="${token%%:*}"
    local inner

    if [[ "$token" == not:* ]]; then
        inner="${token#not:}"
        _nt_complete_query_expr_with_prefix "$inner" "not:"
        return 0
    fi

    if [[ "$token" == \#* ]]; then
        local tag tags candidates
        tags="$(_nt_tag_values)"
        candidates=""
        for tag in $tags; do
            candidates="${candidates} #${tag}"
        done
        COMPREPLY=( $(compgen -W "$candidates" -- "$token") )
        return 0
    fi

    if [[ "$token" != *:* ]]; then
        COMPREPLY=( $(compgen -W "$fields" -- "$token") )
        return 0
    fi

    case "$field" in
        tag) _nt_complete_prefixed_values "$token" tag $(_nt_tag_values) ;;
        collection) _nt_complete_prefixed_values "$token" collection $(nt collections 2>/dev/null) ;;
        kind) _nt_complete_prefixed_values "$token" kind note todo meeting decision source research project ;;
        status) _nt_complete_prefixed_values "$token" status open waiting done dropped ;;
        id) _nt_complete_prefixed_values "$token" id $(nt ids 2>/dev/null) ;;
        link) _nt_complete_prefixed_values "$token" link $(nt ids 2>/dev/null) ;;
        *) COMPREPLY=() ;;
    esac
}

_nt_complete_query_expr_with_prefix() {
    local token="$1"
    local prefix="$2"
    local fields="id: tag: title: day: since: before: kind: status: collection: link: source: body: not:"
    local completions

    _nt_complete_metadata_expr "$token" "$fields"
    completions=("${COMPREPLY[@]}")
    COMPREPLY=()
    local completion
    for completion in "${completions[@]}"; do
        COMPREPLY+=("${prefix}${completion}")
    done
}

_nt_complete_query_expr() {
    _nt_complete_metadata_expr "$(_nt_current_token)" "id: tag: title: day: since: before: kind: status: collection: link: source: body: not:"
}

_nt_complete_add_metadata() {
    _nt_complete_metadata_expr "$(_nt_current_token)" "tag: kind: status: collection: link: source:"
}

_nt() {
    local cur
    if [[ "${BASH_VERSINFO[0]}" -ge 4 ]]; then
        cur="$2"
    else
        cur="${COMP_WORDS[COMP_CWORD]}"
    fi

    case "${COMP_WORDS[1]}:${COMP_CWORD}" in
        find:*)
            _nt_complete_query_expr
            return 0
            ;;
        add:*)
            _nt_complete_add_metadata
            return 0
            ;;
        show:2|edit:2|rm:2|tag:2|untag:2|collect:2|uncollect:2|kind:2|links:2|status:2|link:2|link:3|unlink:2|unlink:3|export:[3-9]|export:[1-9][0-9]*)
            _nt_note_ids
            return 0
            ;;
        tag:3|untag:3)
            _nt_tags
            return 0
            ;;
        collection:2|collect:3|uncollect:3)
            _nt_collections
            return 0
            ;;
        kind:3)
            _nt_kinds
            return 0
            ;;
        status:3)
            _nt_statuses
            return 0
            ;;
        config:3)
            if [[ "${COMP_WORDS[2]}" == "vault" ]]; then
                _nt_vaults
                return 0
            fi
            ;;
    esac

    _nt_clap_complete_generated "$@"
}
"#;

const ZSH_NOTE_ID_COMPLETION: &str = r#"

# nt dynamic completion, backed by visible nt command output.
_nt_note_ids() {
    local -a ids
    ids=("${(@f)$(nt ids 2>/dev/null)}")
    _describe -t note-ids 'note ids' ids "$@"
}

_nt_tags() {
    local -a tags
    tags=("${(@f)$(nt tags 2>/dev/null)}")
    tags=("${(@)tags%%[[:space:]]*}")
    _describe -t tags 'tags' tags "$@"
}

_nt_collections() {
    local -a collections
    collections=("${(@f)$(nt collections 2>/dev/null)}")
    _describe -t collections 'collections' collections "$@"
}

_nt_vaults() {
    local -a lines vaults
    lines=("${(@f)$(nt config vault 2>/dev/null)}")
    vaults=("${(@)${(@)lines#? }%% *}")
    _describe -t vaults 'vaults' vaults "$@"
}

_nt_kinds() {
    local -a kinds
    kinds=(note todo meeting decision source research project)
    _describe -t kinds 'kinds' kinds "$@"
}

_nt_statuses() {
    local -a statuses
    statuses=(open waiting done dropped)
    _describe -t statuses 'statuses' statuses "$@"
}

_nt_sources() {
    local home="${HOME:-${USERPROFILE:-}}"
    local index="${home}/.nt/index.json"
    local line value in_sources=0
    local -a sources

    [[ -r "$index" ]] || return

    while IFS= read -r line; do
        if [[ "$line" == *'"sources": ['* ]]; then
            in_sources=1
            continue
        fi

        if (( in_sources )); then
            if [[ "$line" == *']'* ]]; then
                in_sources=0
                continue
            fi

            value="${line#*\"}"
            value="${value%%\"*}"
            [[ -n "$value" ]] && sources+=("$value")
        fi
    done < "$index"

    typeset -U sources
    print -rl -- "$sources[@]"
}

_nt_complete_prefixed_values() {
    local outer_prefix="$1"
    local field="$2"
    shift 2
    local prefix="${outer_prefix}${field}:"
    local token="$PREFIX"
    local rest list_prefix value_prefix value
    local -a candidates

    [[ "$token" == "$prefix"* ]] || return 1

    rest="${token#$prefix}"
    list_prefix=""
    value_prefix="$rest"
    if [[ "$rest" == *,* ]]; then
        list_prefix="${rest%,*},"
        value_prefix="${rest##*,}"
    fi

    for value in "$@"; do
        if [[ "$value" == "$value_prefix"* ]]; then
            candidates+=("${prefix}${list_prefix}${value}")
        fi
    done

    if (( ${#candidates} == 0 )); then
        compadd -Q -S '' -- "$token"
        return
    fi

    compadd -Q -S '' -a candidates
}

_nt_complete_fields() {
    local outer_prefix="$1"
    shift
    local field
    local -a fields
    for field in "$@"; do
        fields+=("${outer_prefix}${field}")
    done
    compadd -Q -S '' -- "$fields[@]"
}

_nt_query_expr() {
    local token="$PREFIX"
    local outer_prefix=""
    if [[ "$token" == not:* ]]; then
        outer_prefix="not:"
        token="${token#not:}"
    fi
    local field="${token%%:*}"
    local -a fields tags prefixed
    fields=(id: tag: title: day: since: before: kind: status: collection: link: source: body: not:)

    if [[ "$token" == \#* ]]; then
        tags=("${(@f)$(nt tags 2>/dev/null)}")
        tags=("${(@)tags%%[[:space:]]*}")
        compadd -Q -- "${(@/#/#)tags}"
        return
    fi

    if [[ "$token" != *:* ]]; then
        _nt_complete_fields "$outer_prefix" "$fields[@]"
        return
    fi

    case "$field" in
        tag)
            tags=("${(@f)$(nt tags 2>/dev/null)}")
            tags=("${(@)tags%%[[:space:]]*}")
            _nt_complete_prefixed_values "$outer_prefix" tag "$tags[@]"
            ;;
        collection) _nt_complete_prefixed_values "$outer_prefix" collection "${(@f)$(nt collections 2>/dev/null)}" ;;
        kind) _nt_complete_prefixed_values "$outer_prefix" kind note todo meeting decision source research project ;;
        status) _nt_complete_prefixed_values "$outer_prefix" status open waiting done dropped ;;
        id) _nt_complete_prefixed_values "$outer_prefix" id "${(@f)$(nt ids 2>/dev/null)}" ;;
        link) _nt_complete_prefixed_values "$outer_prefix" link "${(@f)$(nt ids 2>/dev/null)}" ;;
        source) _nt_complete_prefixed_values "$outer_prefix" source "${(@f)$(_nt_sources)}" ;;
    esac
}

_nt_add_metadata() {
    local token="$PREFIX"
    local field="${token%%:*}"
    local -a fields tags
    fields=(tag: kind: status: collection: link: source:)

    if [[ "$token" != *:* ]]; then
        _nt_complete_fields "" "$fields[@]"
        return
    fi

    case "$field" in
        tag)
            tags=("${(@f)$(nt tags 2>/dev/null)}")
            tags=("${(@)tags%%[[:space:]]*}")
            _nt_complete_prefixed_values "" tag "$tags[@]"
            ;;
        collection) _nt_complete_prefixed_values "" collection "${(@f)$(nt collections 2>/dev/null)}" ;;
        kind) _nt_complete_prefixed_values "" kind note todo meeting decision source research project ;;
        status) _nt_complete_prefixed_values "" status open waiting done dropped ;;
        link) _nt_complete_prefixed_values "" link "${(@f)$(nt ids 2>/dev/null)}" ;;
        source) _nt_complete_prefixed_values "" source "${(@f)$(_nt_sources)}" ;;
    esac
}
"#;

#[cfg(test)]
mod tests {
    use crate::cli::Shell;

    use super::completion_script;

    #[test]
    fn bash_completion_contains_commands_and_dynamic_note_ids() {
        let script = completion_script(Shell::Bash);

        assert!(script.contains("init add rebuild list find show edit"));
        assert!(script.contains("_nt_note_ids"));
        assert!(script.contains("_nt_complete_query_expr"));
        assert!(script.contains("_nt_complete_add_metadata"));
        assert!(script.contains("nt ids 2>/dev/null"));
        assert!(script.contains("nt tags 2>/dev/null"));
        assert!(script.contains("tag: kind: status: collection: link: source:"));
        assert!(script.contains(
            "id: tag: title: day: since: before: kind: status: collection: link: source: body: not:"
        ));
        assert!(script.contains("show:2|edit:2|rm:2"));
        assert!(script.contains("link:2|link:3|unlink:2|unlink:3"));
        assert!(script.contains("export:[3-9]|export:[1-9][0-9]*"));
    }

    #[test]
    fn zsh_completion_contains_commands_and_dynamic_note_ids() {
        let script = completion_script(Shell::Zsh);

        assert!(script.contains("'show:'"));
        assert!(script.contains(":id:_nt_note_ids"));
        assert!(script.contains(":from_id:_nt_note_ids"));
        assert!(script.contains(":to_id:_nt_note_ids"));
        assert!(script.contains("*::ids:_nt_note_ids"));
        assert!(script.contains("*::metadata:_nt_add_metadata"));
        assert!(script.contains("*::expr:_nt_query_expr"));
        assert!(script.contains(":tag:_nt_tags"));
        assert!(script.contains(":collection:_nt_collections"));
        assert!(script.contains(":kind:_nt_kinds"));
        assert!(script.contains(":status:_nt_statuses"));
        assert!(script.contains("nt ids 2>/dev/null"));
        assert!(script.contains("nt tags 2>/dev/null"));
        assert!(script.contains("_nt_complete_fields"));
        assert!(script.contains("compadd -Q -S '' -- \"$fields[@]\""));
        assert!(script.contains("_nt_sources"));
        assert!(script.contains("source) _nt_complete_prefixed_values"));
        assert!(script.contains("compadd -Q -S '' -a candidates"));

        let helper = script.find("_nt_query_expr()").unwrap();
        let invocation = script.find("_nt \"$@\"").unwrap();
        assert!(helper < invocation);
    }
}
