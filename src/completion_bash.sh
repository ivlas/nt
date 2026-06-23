
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

_nt_complete_link_filter() {
    local token field
    token="$(_nt_current_token)"
    field="${token%%:*}"

    if [[ "$token" != *:* ]]; then
        _nt_complete_list_filter "$token" ""
        COMPREPLY+=( $(compgen -W "from: to:" -- "$token") )
        return
    fi

    case "$field" in
        from|to) _nt_complete_prefixed_values "$token" "$field" $(nt list id 2>/dev/null) ;;
        *) _nt_complete_list_filter "$token" "" ;;
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
        rm:*|update:2|export:[3-9]|export:[1-9][0-9]*)
            _nt_note_ids
            return 0
            ;;
        list:3)
            case "${COMP_WORDS[2]}" in
                links) _nt_complete_link_filter ;;
                tags) COMPREPLY=( $(compgen -W "$(_nt_tag_values)" -- "$(_nt_current_token)") ) ;;
                collections) COMPREPLY=( $(compgen -W "$(_nt_collection_values)" -- "$(_nt_current_token)") ) ;;
                *) _nt_complete_list_arg ;;
            esac
            return 0
            ;;
        list:*)
            if [[ "${COMP_WORDS[2]}" == "links" ]]; then
                _nt_complete_link_filter
            else
                _nt_complete_list_arg
            fi
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
