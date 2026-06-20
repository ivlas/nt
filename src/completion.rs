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
            script = script.replace("*::metadata:_default", "*::metadata:_nt_add_metadata");
            script = script.replace("*::expr:_default", "*::expr:_nt_query_expr");
            script = script.replace("*::args:_default", "*::args:_nt_list_arg");
            script = script.replace(":tag:_default", ":tag:_nt_tags");
            script = script.replace(":collection:_default", ":collection:_nt_collections");
            script = script.replace("'::name:_default'", "'::name:_nt_vaults'");
            script = script.replace(":value:_default", ":value:_nt_update_value");
            script = script.replace("*::ids:_default", "*::ids:_nt_note_ids");
            script = script.replace(
                "(show)\n_arguments \"${_arguments_options[@]}\" : \\\n':id:_nt_note_ids'",
                "(show)\n_arguments \"${_arguments_options[@]}\" : \\\n':id:_nt_titled_notes'",
            );
            script = script.replace(
                "(open)\n_arguments \"${_arguments_options[@]}\" : \\\n':id:_nt_note_ids'",
                "(open)\n_arguments \"${_arguments_options[@]}\" : \\\n':id:_nt_titled_notes'",
            );
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
    COMPREPLY=( $(compgen -W "$(nt list id 2>/dev/null)" -- "${token}") )
}

_nt_titled_notes() {
    local token token_lower id title id_lower title_lower candidates count
    token="$(_nt_current_token)"
    token_lower="$(printf '%s' "$token" | tr '[:upper:]' '[:lower:]')"
    candidates=""
    count=0

    while IFS=$'\t' read -r id title; do
        id_lower="$(printf '%s' "$id" | tr '[:upper:]' '[:lower:]')"
        title_lower="$(printf '%s' "$title" | tr '[:upper:]' '[:lower:]')"
        case "$id_lower:$title_lower" in
            "$token_lower"*|*:"$token_lower"*)
                candidates="${candidates} ${id}"
                count=$((count + 1))
                (( count >= 10 )) && break
                ;;
        esac
    done < <(nt list id,title 2>/dev/null)

    COMPREPLY=( $(compgen -W "$candidates" -- "${token}") )
    if [[ ${#COMPREPLY[@]} -eq 0 && -n "$candidates" ]]; then
        COMPREPLY=( $candidates )
    fi
}

_nt_vaults() {
    local token
    token="$(_nt_current_token)"
    COMPREPLY=( $(compgen -W "$(nt config vault 2>/dev/null | while read -r _ name _; do printf '%s\n' "$name"; done)" -- "${token}") )
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
    nt list tags 2>/dev/null
}

_nt_collection_values() {
    nt list collections 2>/dev/null
}

_nt_source_values() {
    local home="${HOME:-${USERPROFILE:-}}"
    local index="${home}/.nt/index.json"
    local line value in_sources=0

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
            [[ -n "$value" ]] && printf '%s\n' "$value"
        fi
    done < "$index"
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
        collection) _nt_complete_prefixed_values "$token" collection $(_nt_collection_values) ;;
        kind) _nt_complete_prefixed_values "$token" kind note todo meeting decision source research project ;;
        status) _nt_complete_prefixed_values "$token" status open waiting done dropped ;;
        priority) _nt_complete_prefixed_values "$token" priority S A B C D ;;
        id) _nt_complete_prefixed_values "$token" id $(nt list id 2>/dev/null) ;;
        link) _nt_complete_prefixed_values "$token" link $(nt list id 2>/dev/null) ;;
        *) COMPREPLY=() ;;
    esac
}

_nt_complete_query_expr_with_prefix() {
    local token="$1"
    local prefix="$2"
    local fields="id: tag: title: day: since: before: kind: status: priority: scheduled: due: closed: collection: link: source: body: not:"
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
    _nt_complete_metadata_expr "$(_nt_current_token)" "id: tag: title: day: since: before: kind: status: priority: scheduled: due: closed: collection: link: source: body: not:"
}

_nt_complete_list_arg() {
    local token prefix value fields
    token="$(_nt_current_token)"
    fields="id path created updated title kind status priority scheduled due closed tag collection link source"

    if [[ "$COMP_CWORD" -eq 2 && "$token" != *:* && "$token" != \#* ]]; then
        prefix=""
        value="$token"
        if [[ "$token" == *,* ]]; then
            prefix="${token%,*},"
            value="${token##*,}"
        fi
        COMPREPLY=( $(compgen -W "$(for field in $fields; do printf '%s\n' "${prefix}${field}"; done)" -- "$token") )
        if [[ -z "$prefix" ]]; then
            COMPREPLY+=( $(compgen -W "all ids titles tags collections links" -- "$token") )
        fi
        return
    fi

    _nt_complete_list_filter "$token" ""
}

_nt_complete_list_filter() {
    local token="$1"
    local outer_prefix="$2"
    local field tags candidates tag
    local fields="id: tag: day: since: before: kind: status: priority: scheduled: due: closed: collection: link: not:"

    if [[ "$token" == not:* ]]; then
        _nt_complete_list_filter "${token#not:}" "${outer_prefix}not:"
        return
    fi
    if [[ "$token" == \#* ]]; then
        tags="$(_nt_tag_values)"
        for tag in $tags; do candidates="${candidates} ${outer_prefix}#${tag}"; done
        COMPREPLY=( $(compgen -W "$candidates" -- "$(_nt_current_token)") )
        return
    fi
    if [[ "$token" != *:* ]]; then
        for field in $fields; do candidates="${candidates} ${outer_prefix}${field}"; done
        COMPREPLY=( $(compgen -W "$candidates" -- "$(_nt_current_token)") )
        return
    fi

    field="${token%%:*}"
    case "$field" in
        tag) _nt_complete_prefixed_values "$(_nt_current_token)" "${outer_prefix}tag" $(_nt_tag_values) ;;
        collection) _nt_complete_prefixed_values "$(_nt_current_token)" "${outer_prefix}collection" $(_nt_collection_values) ;;
        kind) _nt_complete_prefixed_values "$(_nt_current_token)" "${outer_prefix}kind" note todo meeting decision source research project ;;
        status) _nt_complete_prefixed_values "$(_nt_current_token)" "${outer_prefix}status" open waiting done dropped ;;
        priority) _nt_complete_prefixed_values "$(_nt_current_token)" "${outer_prefix}priority" S A B C D ;;
        id) _nt_complete_prefixed_values "$(_nt_current_token)" "${outer_prefix}id" $(nt list id 2>/dev/null) ;;
        link) _nt_complete_prefixed_values "$(_nt_current_token)" "${outer_prefix}link" $(nt list id 2>/dev/null) ;;
    esac
}

_nt_complete_add_metadata() {
    _nt_complete_metadata_expr "$(_nt_current_token)" "tag: kind: status: priority: scheduled: due: collection: link: source:"
}

_nt_complete_update_set_values() {
    local token value candidates
    token="$(_nt_current_token)"
    shift
    candidates=""
    for value in "$@"; do
        candidates="${candidates} +${value} -${value}"
    done
    COMPREPLY=( $(compgen -W "$candidates" -- "$token") )
}

_nt_update_value() {
    case "${COMP_WORDS[3]}" in
        kind) COMPREPLY=( $(compgen -W "note todo meeting decision source research project -" -- "$(_nt_current_token)") ) ;;
        status) COMPREPLY=( $(compgen -W "open waiting done dropped -" -- "$(_nt_current_token)") ) ;;
        priority) COMPREPLY=( $(compgen -W "S A B C D -" -- "$(_nt_current_token)") ) ;;
        scheduled|due) COMPREPLY=( $(compgen -W "-" -- "$(_nt_current_token)") ) ;;
        tag) _nt_complete_update_set_values tags $(_nt_tag_values) ;;
        collection) _nt_complete_update_set_values collections $(_nt_collection_values) ;;
        link) _nt_complete_update_set_values links $(nt list id 2>/dev/null) ;;
        source) _nt_complete_update_set_values sources $(_nt_source_values) ;;
    esac
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
        show:2|open:2)
            _nt_titled_notes
            return 0
            ;;
        rm:2|update:2|export:[3-9]|export:[1-9][0-9]*)
            _nt_note_ids
            return 0
            ;;
        list:3)
            case "${COMP_WORDS[2]}" in
                links) _nt_note_ids ;;
                tags) COMPREPLY=( $(compgen -W "$(_nt_tag_values)" -- "$(_nt_current_token)") ) ;;
                collections) COMPREPLY=( $(compgen -W "$(_nt_collection_values)" -- "$(_nt_current_token)") ) ;;
                *) _nt_complete_list_arg ;;
            esac
            return 0
            ;;
        list:*)
            _nt_complete_list_arg
            return 0
            ;;
        update:4)
            _nt_update_value
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
    ids=("${(@f)$(command nt list id 2>/dev/null)}")
    _describe -t note-ids 'note ids' ids "$@"
}

_nt_titled_notes() {
    local token="${PREFIX:l}"
    local id title
    local -a ids displays

    while IFS=$'\t' read -r id title; do
        if [[ "${id:l}" == "$token"* || "${title:l}" == "$token"* ]]; then
            ids+=("$id")
            displays+=("$id $title")
            (( ${#ids} >= 10 )) && break
        fi
    done < <(command nt list id,title 2>/dev/null)

    (( ${#ids} > 0 )) || return
    compadd -Q -S '' -U -d displays -a ids
    if (( ${#ids} > 1 )); then
        if [[ -n "$compstate[old_list]" ]]; then
            compstate[insert]=menu
        elif [[ "$compstate[insert]" != menu ]]; then
            compstate[insert]=
        fi
    fi
}

_nt_tag_values() {
    command nt list tags 2>/dev/null
}

_nt_tags() {
    local -a tags
    tags=("${(@f)$(_nt_tag_values)}")
    _describe -t tags 'tags' tags "$@"
}

_nt_collection_values() {
    command nt list collections 2>/dev/null
}

_nt_collections() {
    local -a collections
    collections=("${(@f)$(_nt_collection_values)}")
    _describe -t collections 'collections' collections "$@"
}

_nt_vaults() {
    local -a lines vaults
    lines=("${(@f)$(command nt config vault 2>/dev/null)}")
    vaults=("${(@)${(@)lines#? }%% *}")
    _describe -t vaults 'vaults' vaults "$@"
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
    local token="${IPREFIX}${PREFIX}"
    local rest list_prefix value_prefix value completion_prefix
    local -a candidates completions

    [[ -n "$token" ]] || token="${words[CURRENT]}"

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
            candidates+=("$value")
        fi
    done

    completion_prefix="${prefix}${list_prefix}"

    (( ${#candidates} > 0 )) || return

    if compset -P "$completion_prefix"; then
        compadd -Q -S '' -a candidates
    elif [[ "$IPREFIX" == "$completion_prefix" ]]; then
        compadd -Q -S '' -a candidates
    else
        completions=("${(@)candidates/#/${completion_prefix}}")
        compadd -Q -S '' -U -a completions
    fi
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
    local token="${IPREFIX}${PREFIX}"
    local outer_prefix=""
    [[ -n "$token" ]] || token="${words[CURRENT]}"
    if [[ "$token" == not:* ]]; then
        outer_prefix="not:"
        token="${token#not:}"
    fi
    local field="${token%%:*}"
    local -a fields tags prefixed
    fields=(id: tag: title: day: since: before: kind: status: priority: scheduled: due: closed: collection: link: source: body: not:)

    if [[ "$token" == \#* ]]; then
        tags=("${(@f)$(_nt_tag_values)}")
        compadd -Q -- "${(@/#/#)tags}"
        return
    fi

    if [[ "$token" != *:* ]]; then
        _nt_complete_fields "$outer_prefix" "$fields[@]"
        return
    fi

    case "$field" in
        tag)
            tags=("${(@f)$(_nt_tag_values)}")
            _nt_complete_prefixed_values "$outer_prefix" tag "$tags[@]"
            ;;
        collection) _nt_complete_prefixed_values "$outer_prefix" collection "${(@f)$(_nt_collection_values)}" ;;
        kind) _nt_complete_prefixed_values "$outer_prefix" kind note todo meeting decision source research project ;;
        status) _nt_complete_prefixed_values "$outer_prefix" status open waiting done dropped ;;
        priority) _nt_complete_prefixed_values "$outer_prefix" priority S A B C D ;;
        id) _nt_complete_prefixed_values "$outer_prefix" id "${(@f)$(command nt list id 2>/dev/null)}" ;;
        link) _nt_complete_prefixed_values "$outer_prefix" link "${(@f)$(command nt list id 2>/dev/null)}" ;;
        source) _nt_complete_prefixed_values "$outer_prefix" source "${(@f)$(_nt_sources)}" ;;
    esac
}

_nt_add_metadata() {
    local token="${IPREFIX}${PREFIX}"
    [[ -n "$token" ]] || token="${words[CURRENT]}"
    local field="${token%%:*}"
    local -a fields tags
    fields=(tag: kind: status: priority: scheduled: due: collection: link: source:)

    if [[ "$token" != *:* ]]; then
        _nt_complete_fields "" "$fields[@]"
        return
    fi

    case "$field" in
        tag)
            tags=("${(@f)$(_nt_tag_values)}")
            _nt_complete_prefixed_values "" tag "$tags[@]"
            ;;
        collection) _nt_complete_prefixed_values "" collection "${(@f)$(_nt_collection_values)}" ;;
        kind) _nt_complete_prefixed_values "" kind note todo meeting decision source research project ;;
        status) _nt_complete_prefixed_values "" status open waiting done dropped ;;
        priority) _nt_complete_prefixed_values "" priority S A B C D ;;
        link) _nt_complete_prefixed_values "" link "${(@f)$(command nt list id 2>/dev/null)}" ;;
        source) _nt_complete_prefixed_values "" source "${(@f)$(_nt_sources)}" ;;
    esac
}

_nt_list_arg() {
    local token="${IPREFIX}${PREFIX}"
    [[ -n "$token" ]] || token="${words[CURRENT]}"
    local prefix=""
    local value="$token"
    local field
    local -a fields candidates
    fields=(id path created updated title kind status priority scheduled due closed tag collection link source)

    if (( CURRENT == 3 )) && [[ "$token" != *:* && "$token" != \#* ]]; then
        if [[ "$token" == *,* ]]; then
            prefix="${token%,*},"
            value="${token##*,}"
        fi
        for field in "$fields[@]"; do
            [[ "$field" == "$value"* ]] && candidates+=("${prefix}${field}")
        done
        [[ -z "$prefix" ]] && candidates+=(all ids titles tags collections links)
        compadd -Q -S '' -U -a candidates
        return
    fi

    local outer_prefix=""
    if [[ "$token" == not:* ]]; then
        outer_prefix="not:"
        token="${token#not:}"
    fi
    local filter_field="${token%%:*}"
    local -a filter_fields tags
    filter_fields=(id: tag: day: since: before: kind: status: priority: scheduled: due: closed: collection: link: not:)

    if [[ "$token" == \#* ]]; then
        tags=("${(@f)$(_nt_tag_values)}")
        compadd -Q -- "${(@)tags/#/${outer_prefix}#}"
    elif [[ "$token" != *:* ]]; then
        _nt_complete_fields "$outer_prefix" "$filter_fields[@]"
    else
        case "$filter_field" in
            tag) _nt_complete_prefixed_values "$outer_prefix" tag "${(@f)$(_nt_tag_values)}" ;;
            collection) _nt_complete_prefixed_values "$outer_prefix" collection "${(@f)$(_nt_collection_values)}" ;;
            kind) _nt_complete_prefixed_values "$outer_prefix" kind note todo meeting decision source research project ;;
            status) _nt_complete_prefixed_values "$outer_prefix" status open waiting done dropped ;;
            priority) _nt_complete_prefixed_values "$outer_prefix" priority S A B C D ;;
            id) _nt_complete_prefixed_values "$outer_prefix" id "${(@f)$(command nt list id 2>/dev/null)}" ;;
            link) _nt_complete_prefixed_values "$outer_prefix" link "${(@f)$(command nt list id 2>/dev/null)}" ;;
        esac
    fi
}

_nt_complete_update_set_values() {
    local description="$1"
    shift
    local value
    local -a candidates
    for value in "$@"; do
        candidates+=("+${value}" "-${value}")
    done
    _describe -t update-values "$description" candidates
}

_nt_update_value() {
    case "$words[4]" in
        kind) _values kinds note todo meeting decision source research project - ;;
        status) _values statuses open waiting done dropped - ;;
        priority) _values priorities S A B C D - ;;
        scheduled|due) _values dates - ;;
        tag) _nt_complete_update_set_values tags "${(@f)$(_nt_tag_values)}" ;;
        collection) _nt_complete_update_set_values collections "${(@f)$(_nt_collection_values)}" ;;
        link) _nt_complete_update_set_values links "${(@f)$(command nt list id 2>/dev/null)}" ;;
        source) _nt_complete_update_set_values sources "${(@f)$(_nt_sources)}" ;;
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

        assert!(script.contains("init add rebuild list find show open"));
        assert!(script.contains("_nt_note_ids"));
        assert!(script.contains("_nt_complete_query_expr"));
        assert!(script.contains("_nt_complete_add_metadata"));
        assert!(script.contains("nt list id 2>/dev/null"));
        assert!(script.contains("nt list tags 2>/dev/null"));
        assert!(
            script
                .contains("tag: kind: status: priority: scheduled: due: collection: link: source:")
        );
        assert!(script.contains(
            "id: tag: title: day: since: before: kind: status: priority: scheduled: due: closed: collection: link: source: body: not:"
        ));
        assert!(script.contains("show:2|open:2"));
        assert!(script.contains("nt list id,title 2>/dev/null"));
        assert!(script.contains("_nt_complete_list_arg"));
        assert!(script.contains("_nt_complete_list_filter"));
        assert!(script.contains("id path created updated title kind status"));
        let list_filter = script.split("_nt_complete_list_filter() {").nth(1).unwrap();
        let list_filter = list_filter
            .split("_nt_complete_add_metadata() {")
            .next()
            .unwrap();
        assert!(!list_filter.contains("body:"));
        assert!(!list_filter.contains("title:"));
        assert!(script.contains("(( count >= 10 )) && break"));
        assert!(script.contains("update:4"));
        assert!(script.contains("S A B C D -"));
        assert!(
            script.contains("priority) _nt_complete_prefixed_values \"$token\" priority S A B C D")
        );
        assert!(script.contains("_nt_complete_update_set_values"));
        assert!(script.contains("candidates=\"${candidates} +${value} -${value}\""));
        assert!(script.contains("source) _nt_complete_update_set_values sources"));
        assert!(script.contains("list:3"));
        assert!(script.contains("links) _nt_note_ids"));
        assert!(script.contains("export:[3-9]|export:[1-9][0-9]*"));
    }

    #[test]
    fn zsh_completion_contains_commands_and_dynamic_note_ids() {
        let script = completion_script(Shell::Zsh);

        assert!(script.contains("'show:'"));
        assert!(script.contains("'open:'"));
        assert!(script.contains(
            "(show)\n_arguments \"${_arguments_options[@]}\" : \\\n':id:_nt_titled_notes'"
        ));
        assert!(script.contains(
            "(open)\n_arguments \"${_arguments_options[@]}\" : \\\n':id:_nt_titled_notes'"
        ));
        assert!(script.contains("command nt list id,title 2>/dev/null"));
        assert!(script.contains("displays+=(\"$id $title\")"));
        assert!(script.contains("(( ${#ids} >= 10 )) && break"));
        assert!(script.contains("(( ${#ids} > 1 ))"));
        assert!(script.contains("[[ -n \"$compstate[old_list]\" ]]"));
        assert!(script.contains("compstate[insert]=menu"));
        assert!(script.contains("compstate[insert]="));
        assert!(script.contains(":id:_nt_note_ids"));
        assert!(script.contains(":value:_nt_update_value"));
        assert!(script.contains("*::metadata:_nt_add_metadata"));
        assert!(script.contains("*::expr:_nt_query_expr"));
        assert!(script.contains("_nt_tag_values"));
        assert!(script.contains("_nt_collection_values"));
        assert!(script.contains("_nt_tags()"));
        assert!(script.contains("_nt_collections()"));
        assert!(!script.contains(":tag:_nt_tags"));
        assert!(!script.contains(":collection:_nt_collections"));
        assert!(!script.contains(":from_id:_nt_note_ids"));
        assert!(!script.contains(":to_id:_nt_note_ids"));
        assert!(!script.contains("_nt_kinds"));
        assert!(!script.contains("_nt_statuses"));
        assert!(!script.contains("lines#*$'\\t'"));
        assert!(script.contains("S A B C D -"));
        assert!(script.contains(
            "priority) _nt_complete_prefixed_values \"$outer_prefix\" priority S A B C D"
        ));
        assert!(script.contains("candidates+=(\"+${value}\" \"-${value}\")"));
        assert!(script.contains("source) _nt_complete_update_set_values sources"));
        assert!(script.contains("nt list id 2>/dev/null"));
        assert!(script.contains("nt list tags 2>/dev/null"));
        assert!(script.contains("_nt_complete_fields"));
        assert!(script.contains("compadd -Q -S '' -- \"$fields[@]\""));
        assert!(script.contains("_nt_sources"));
        assert!(script.contains("source) _nt_complete_prefixed_values"));
        assert!(script.contains("local token=\"${IPREFIX}${PREFIX}\""));
        assert!(script.contains("[[ \"$IPREFIX\" == \"$completion_prefix\" ]]"));
        assert!(script.contains("compadd -Q -S '' -U -a completions"));
        assert!(script.contains("*::args:_nt_list_arg"));
        assert!(script.contains("_nt_list_arg()"));
        let list_arg = script.split("_nt_list_arg() {").nth(1).unwrap();
        let list_arg = list_arg
            .split("_nt_complete_update_set_values() {")
            .next()
            .unwrap();
        assert!(!list_arg.contains("body:"));
        assert!(!list_arg.contains("title:"));

        let helper = script.find("_nt_query_expr()").unwrap();
        let invocation = script.find("_nt \"$@\"").unwrap();
        assert!(helper < invocation);
    }

    #[test]
    fn zsh_prefixed_value_completion_normalizes_the_prefix_once() {
        let script = completion_script(Shell::Zsh);
        let start = script.find("_nt_complete_prefixed_values()").unwrap();
        let end = script[start..]
            .find("\n}\n\n_nt_complete_fields()")
            .unwrap()
            + start;
        let helper = &script[start..end];

        assert_eq!(
            helper.matches("compset -P \"$completion_prefix\"").count(),
            1
        );
        assert!(helper.contains("token=\"${IPREFIX}${PREFIX}\""));
        assert!(helper.contains("token=\"${words[CURRENT]}\""));
        assert!(helper.contains("compadd -Q -S '' -a candidates"));
        assert!(helper.contains("completions=(\"${(@)candidates/#/${completion_prefix}}\")"));
    }
}
