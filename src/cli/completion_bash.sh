# nt dynamic completion, backed by visible nt command output.
eval "$(declare -f _nt | sed '1s/^_nt/_nt_clap_complete_generated/')"

_nt_current_token() {
    local line="${COMP_LINE:0:COMP_POINT}"
    local token="${line##* }"
    token="${token##*$'\t'}"
    printf '%s' "$token"
}

_nt_quote_completion() {
    local quoted
    printf -v quoted '%q' "$1"
    printf '%s' "$quoted"
}

_nt_note_ids() {
    local token
    token="$(_nt_current_token)"
    local -a ids=()
    mapfile -t ids < <(command nt list id 2>/dev/null)
    COMPREPLY=()
    local id
    for id in "${ids[@]}"; do
        [[ "$id" == "$token"* ]] && COMPREPLY+=("$id")
    done
}

_nt_titled_notes() {
    local token token_lower id title id_lower title_lower count
    token="$(_nt_current_token)"
    token_lower="$(printf '%s' "$token" | tr '[:upper:]' '[:lower:]')"
    COMPREPLY=()
    count=0

    while IFS=$'\t' read -r id title; do
        id_lower="$(printf '%s' "$id" | tr '[:upper:]' '[:lower:]')"
        title_lower="$(printf '%s' "$title" | tr '[:upper:]' '[:lower:]')"
        case "$id_lower:$title_lower" in
            "$token_lower"*|*:"$token_lower"*)
                COMPREPLY+=("$id")
                count=$((count + 1))
                (( count >= 10 )) && break
                ;;
        esac
    done < <(command nt list id,title 2>/dev/null)

    if [[ ${#COMPREPLY[@]} -eq 0 ]]; then
        local candidates=""
        while IFS=$'\t' read -r id title; do
            id_lower="$(printf '%s' "$id" | tr '[:upper:]' '[:lower:]')"
            title_lower="$(printf '%s' "$title" | tr '[:upper:]' '[:lower:]')"
            case "$id_lower:$title_lower" in
                "$token_lower"*|*:"$token_lower"*) candidates="${candidates} ${id}" ;;
            esac
        done < <(command nt list id,title 2>/dev/null)
        [[ -n "$candidates" ]] && mapfile -t COMPREPLY < <(printf '%s\n' $candidates)
    fi
}

_nt_vaults() {
    local token
    token="$(_nt_current_token)"
    local -a vaults=()
    mapfile -t vaults < <(command nt config vault 2>/dev/null | while read -r _ name _; do printf '%s\n' "$name"; done)
    COMPREPLY=()
    local vault
    for vault in "${vaults[@]}"; do
        [[ "$vault" == "$token"* ]] && COMPREPLY+=("$(_nt_quote_completion "$vault")")
    done
}

_nt_complete_prefixed_values() {
    local token="$1"
    local field="$2"
    shift 2
    local prefix="${field}:"
    local value_prefix value
    local -a candidates=()

    [[ "$token" == "$prefix"* ]] || return 1

    value_prefix="${token#"$prefix"}"

    for value in "$@"; do
        if [[ "$value" == "$value_prefix"* ]]; then
            candidates+=("$(_nt_quote_completion "${prefix}${value}")")
        fi
    done
    COMPREPLY=("${candidates[@]}")
    return 0
}

_nt_complete_prefixed_list_values() {
    local token="$1"
    local field="$2"
    shift 2
    local prefix="${field}:"
    local rest list_prefix value_prefix value
    local -a candidates=()

    [[ "$token" == "$prefix"* ]] || return 1

    rest="${token#"$prefix"}"
    list_prefix=""
    value_prefix="$rest"
    if [[ "$rest" == *,* ]]; then
        list_prefix="${rest%,*},"
        value_prefix="${rest##*,}"
    fi

    for value in "$@"; do
        if [[ "$value" == "$value_prefix"* ]]; then
            candidates+=("$(_nt_quote_completion "${prefix}${list_prefix}${value}")")
        fi
    done
    COMPREPLY=("${candidates[@]}")
    return 0
}

_nt_tag_values() {
    command nt list tags 2>/dev/null
}

_nt_collection_values() {
    command nt list collections 2>/dev/null
}

_nt_source_values() {
    command nt list sources 2>/dev/null
}

_nt_complete_raw_values() {
    local -a values=()
    mapfile -t values < <("$1" 2>/dev/null)
    local token="$(_nt_current_token)" value
    COMPREPLY=()
    for value in "${values[@]}"; do
        [[ "$value" == "$token"* ]] && COMPREPLY+=("$(_nt_quote_completion "$value")")
    done
}

_nt_complete_metadata_expr() {
    local token="$1"
    local fields="$2"
    local field="${token%%:*}"
    local inner
    local -a tags ids sources

    if [[ "$token" == not:* ]]; then
        inner="${token#not:}"
        _nt_complete_query_expr_with_prefix "$inner" "not:"
        return 0
    fi

    if [[ "$token" == \#* ]]; then
        mapfile -t tags < <(_nt_tag_values)
        local -a candidates=()
        local tag
        for tag in "${tags[@]}"; do
            candidates+=("#${tag}")
        done
        COMPREPLY=()
        for tag in "${candidates[@]}"; do
            [[ "$tag" == "$token"* ]] && COMPREPLY+=("$(_nt_quote_completion "$tag")")
        done
        return 0
    fi

    if [[ "$token" != *:* ]]; then
        mapfile -t COMPREPLY < <(compgen -W "$fields" -- "$token")
        return 0
    fi

    case "$field" in
        tag) mapfile -t tags < <(_nt_tag_values); _nt_complete_prefixed_values "$token" tag "${tags[@]}" ;;
        collection) mapfile -t tags < <(_nt_collection_values); _nt_complete_prefixed_values "$token" collection "${tags[@]}" ;;
        kind) _nt_complete_prefixed_values "$token" kind note todo ;;
        status) _nt_complete_prefixed_values "$token" status open waiting done dropped ;;
        priority) _nt_complete_prefixed_values "$token" priority S A B C D ;;
        id) mapfile -t ids < <(command nt list id 2>/dev/null); _nt_complete_prefixed_values "$token" id "${ids[@]}" ;;
        link) mapfile -t ids < <(command nt list id 2>/dev/null); _nt_complete_prefixed_values "$token" link "${ids[@]}" ;;
        source) mapfile -t sources < <(_nt_source_values); _nt_complete_prefixed_values "$token" source "${sources[@]}" ;;
        *) COMPREPLY=() ;;
    esac
}

_nt_complete_query_expr_with_prefix() {
    local token="$1"
    local prefix="$2"
    local fields="id: tag: title: day: since: before: kind: status: priority: scheduled: due: closed: collection: link: source: body: not:"
    local -a completions
    local completion

    _nt_complete_metadata_expr "$token" "$fields"
    completions=("${COMPREPLY[@]}")
    COMPREPLY=()
    for completion in "${completions[@]}"; do
        COMPREPLY+=("${prefix}${completion}")
    done
}

_nt_complete_query_expr() {
    _nt_complete_metadata_expr "$(_nt_current_token)" "id: tag: title: day: since: before: kind: status: priority: scheduled: due: closed: collection: link: source: body: not:"
}

_nt_complete_list_arg() {
    local token prefix value field
    local -a fields
    token="$(_nt_current_token)"
    fields=(id path created updated title kind status priority scheduled due closed tag collection link source)

    if [[ "$COMP_CWORD" -eq 2 && "$token" != *:* && "$token" != \#* ]]; then
        prefix=""
        value="$token"
        if [[ "$token" == *,* ]]; then
            prefix="${token%,*},"
            value="${token##*,}"
        fi
        COMPREPLY=()
        for field in "${fields[@]}"; do
            [[ "$field" == "$value"* ]] && COMPREPLY+=("${prefix}${field}")
        done
        if [[ -z "$prefix" ]]; then
            for field in all ids titles tags collections links; do
                [[ "$field" == "$value"* ]] && COMPREPLY+=("$field")
            done
        fi
        return
    fi

    _nt_complete_list_filter "$token" ""
}

_nt_complete_list_filter() {
    local token="$1"
    local outer_prefix="$2"
    local field tag
    local -a tags ids
    local fields=(id: tag: day: since: before: kind: status: priority: scheduled: due: closed: collection: link: not:)

    if [[ "$token" == not:* ]]; then
        _nt_complete_list_filter "${token#not:}" "${outer_prefix}not:"
        return
    fi
    if [[ "$token" == \#* ]]; then
        mapfile -t tags < <(_nt_tag_values)
        COMPREPLY=()
        for tag in "${tags[@]}"; do
            [[ "${outer_prefix}#${tag}" == "$(_nt_current_token)"* ]] && COMPREPLY+=("$(_nt_quote_completion "${outer_prefix}#${tag}")")
        done
        return
    fi
    if [[ "$token" != *:* ]]; then
        COMPREPLY=()
        for field in "${fields[@]}"; do
            COMPREPLY+=("${outer_prefix}${field}")
        done
        return
    fi

    field="${token%%:*}"
    case "$field" in
        tag) mapfile -t tags < <(_nt_tag_values); _nt_complete_prefixed_values "$(_nt_current_token)" "${outer_prefix}tag" "${tags[@]}" ;;
        collection) mapfile -t tags < <(_nt_collection_values); _nt_complete_prefixed_values "$(_nt_current_token)" "${outer_prefix}collection" "${tags[@]}" ;;
        kind) _nt_complete_prefixed_values "$(_nt_current_token)" "${outer_prefix}kind" note todo ;;
        status) _nt_complete_prefixed_values "$(_nt_current_token)" "${outer_prefix}status" open waiting done dropped ;;
        priority) _nt_complete_prefixed_values "$(_nt_current_token)" "${outer_prefix}priority" S A B C D ;;
        id) mapfile -t ids < <(command nt list id 2>/dev/null); _nt_complete_prefixed_values "$(_nt_current_token)" "${outer_prefix}id" "${ids[@]}" ;;
        link) mapfile -t ids < <(command nt list id 2>/dev/null); _nt_complete_prefixed_values "$(_nt_current_token)" "${outer_prefix}link" "${ids[@]}" ;;
    esac
}

_nt_complete_link_filter() {
    local token field
    local -a ids
    token="$(_nt_current_token)"
    field="${token%%:*}"

    if [[ "$token" != *:* ]]; then
        _nt_complete_list_filter "$token" ""
        for field in from: to:; do
            [[ "$field" == "$token"* ]] && COMPREPLY+=("$field")
        done
        return
    fi

    case "$field" in
        from|to) mapfile -t ids < <(command nt list id 2>/dev/null); _nt_complete_prefixed_values "$token" "$field" "${ids[@]}" ;;
        *) _nt_complete_list_filter "$token" "" ;;
    esac
}

_nt_complete_add_metadata() {
    local token field
    local -a tags ids sources
    token="$(_nt_current_token)"
    field="${token%%:*}"

    local fields="tag: collection: link: source:"
    if [[ "${COMP_WORDS[1]}" == "todo" ]]; then
        fields="status: priority: scheduled: due: tag: collection: link: source:"
    fi

    if [[ "$token" != *:* || "$token" == not:* || "$token" == \#* ]]; then
        _nt_complete_metadata_expr "$token" "$fields"
        return
    fi

    case "$field" in
        tag) mapfile -t tags < <(_nt_tag_values); _nt_complete_prefixed_list_values "$token" tag "${tags[@]}" ;;
        collection) mapfile -t tags < <(_nt_collection_values); _nt_complete_prefixed_list_values "$token" collection "${tags[@]}" ;;
        link) mapfile -t ids < <(command nt list id 2>/dev/null); _nt_complete_prefixed_list_values "$token" link "${ids[@]}" ;;
        source) mapfile -t sources < <(_nt_source_values); _nt_complete_prefixed_values "$token" source "${sources[@]}" ;;
        status) [[ "${COMP_WORDS[1]}" == "todo" ]] && _nt_complete_prefixed_values "$token" status open waiting done dropped ;;
        priority) [[ "${COMP_WORDS[1]}" == "todo" ]] && _nt_complete_prefixed_values "$token" priority S A B C D ;;
        *) _nt_complete_metadata_expr "$token" "$fields" ;;
    esac
}

_nt_complete_update_set_values() {
    local token value
    local -a all=("$@") candidates=()
    token="$(_nt_current_token)"
    for value in "${all[@]}"; do
        [[ "+${value}" == "$token"* ]] && candidates+=("$(_nt_quote_completion "+${value}")")
        [[ "-${value}" == "$token"* ]] && candidates+=("$(_nt_quote_completion "-${value}")")
    done
    COMPREPLY=("${candidates[@]}")
}

_nt_update_value() {
    local token
    local -a tags ids sources
    token="$(_nt_current_token)"
    case "${COMP_WORDS[3]}" in
        kind) COMPREPLY=(); for value in note todo -; do [[ "$value" == "$token"* ]] && COMPREPLY+=("$value"); done ;;
        status) COMPREPLY=(); for value in open waiting done dropped -; do [[ "$value" == "$token"* ]] && COMPREPLY+=("$value"); done ;;
        priority) COMPREPLY=(); for value in S A B C D -; do [[ "$value" == "$token"* ]] && COMPREPLY+=("$value"); done ;;
        scheduled|due) COMPREPLY=(); [[ "-" == "$token"* ]] && COMPREPLY+=("-") ;;
        tag) mapfile -t tags < <(_nt_tag_values); _nt_complete_update_set_values "${tags[@]}" ;;
        collection) mapfile -t tags < <(_nt_collection_values); _nt_complete_update_set_values "${tags[@]}" ;;
        link) mapfile -t ids < <(command nt list id 2>/dev/null); _nt_complete_update_set_values "${ids[@]}" ;;
        source) mapfile -t sources < <(_nt_source_values); _nt_complete_update_set_values "${sources[@]}" ;;
    esac
}

_nt() {
    local cur
    local -a ids
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
        note:*|todo:*)
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
                tags) _nt_complete_raw_values _nt_tag_values ;;
                collections) _nt_complete_raw_values _nt_collection_values ;;
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
