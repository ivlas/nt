
# Complete note ids from visible nt output.
_nt_note_ids() {
    local -a ids
    ids=("${(@f)$(command nt list id 2>/dev/null)}")
    _describe -t note-ids 'note ids' ids "$@"
}
