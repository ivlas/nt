
# Complete note ids from visible nt output; delegate everything else to clap.
eval "$(declare -f _nt | sed '1s/^_nt/_nt_clap_complete_generated/')"

_nt_note_ids() {
    COMPREPLY=($(compgen -W "$(command nt list id 2>/dev/null)" -- "$2"))
}

_nt() {
    case "${COMP_WORDS[1]}:${COMP_CWORD}" in
        show:2|open:2|rm:*|update:2|export:[3-9]|export:[1-9][0-9]*)
            _nt_note_ids "$@"
            return 0
            ;;
    esac

    _nt_clap_complete_generated "$@"
}
