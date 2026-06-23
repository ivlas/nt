
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

_nt_link_filter_arg() {
    local token="${IPREFIX}${PREFIX}"
    [[ -n "$token" ]] || token="${words[CURRENT]}"
    local outer_prefix=""
    if [[ "$token" == not:* ]]; then
        outer_prefix="not:"
        token="${token#not:}"
    fi
    local field="${token%%:*}"
    local -a fields tags
    fields=(id: tag: day: since: before: kind: status: priority: scheduled: due: closed: collection: link: not:)
    [[ -z "$outer_prefix" ]] && fields+=(from: to:)

    if [[ "$token" == \#* ]]; then
        tags=("${(@f)$(_nt_tag_values)}")
        compadd -Q -- "${(@)tags/#/${outer_prefix}#}"
    elif [[ "$token" != *:* ]]; then
        _nt_complete_fields "$outer_prefix" "$fields[@]"
    else
        case "$field" in
            from|to) _nt_complete_prefixed_values "" "$field" "${(@f)$(command nt list id 2>/dev/null)}" ;;
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

_nt_list_arg() {
    local token="${IPREFIX}${PREFIX}"
    [[ -n "$token" ]] || token="${words[CURRENT]}"
    local prefix=""
    local value="$token"
    local field
    local -a fields candidates
    fields=(id path created updated title kind status priority scheduled due closed tag collection link source)

    if [[ "${words[3]}" == "links" ]]; then
        _nt_link_filter_arg
        return
    fi

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
