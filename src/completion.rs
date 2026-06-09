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
            script = script.replace(
                "*::args:_default",
                ":id:_nt_note_ids' \\\n':status:_default",
            );
            script = script.replace("*::ids:_default", "*::ids:_nt_note_ids");
            script.push_str(ZSH_NOTE_ID_COMPLETION);
        }
    }

    script
}

const BASH_NOTE_ID_COMPLETION: &str = r#"

# nt dynamic note id completion, backed by visible `nt ids` output.
eval "$(declare -f _nt | sed '1s/^_nt/_nt_clap_complete_generated/')"

_nt_note_ids() {
    COMPREPLY=( $(compgen -W "$(nt ids 2>/dev/null)" -- "${cur}") )
}

_nt() {
    local cur
    if [[ "${BASH_VERSINFO[0]}" -ge 4 ]]; then
        cur="$2"
    else
        cur="${COMP_WORDS[COMP_CWORD]}"
    fi

    case "${COMP_WORDS[1]}:${COMP_CWORD}" in
        show:2|edit:2|rm:2|tag:2|untag:2|collect:2|uncollect:2|kind:2|links:2|status:2|link:2|link:3|unlink:2|unlink:3|export:[3-9]|export:[1-9][0-9]*)
            _nt_note_ids
            return 0
            ;;
    esac

    _nt_clap_complete_generated "$@"
}
"#;

const ZSH_NOTE_ID_COMPLETION: &str = r#"

# nt dynamic note id completion, backed by visible `nt ids` output.
_nt_note_ids() {
    local -a ids
    ids=("${(@f)$(nt ids 2>/dev/null)}")
    _describe -t note-ids 'note ids' ids "$@"
}
"#;

#[cfg(test)]
mod tests {
    use crate::cli::Shell;

    use super::completion_script;

    #[test]
    fn bash_completion_contains_commands_and_dynamic_note_ids() {
        let script = completion_script(Shell::Bash);

        assert!(script.contains("init add list find show edit"));
        assert!(script.contains("_nt_note_ids"));
        assert!(script.contains("nt ids 2>/dev/null"));
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
        assert!(script.contains("nt ids 2>/dev/null"));
    }
}
