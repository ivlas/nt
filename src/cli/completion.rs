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
        Shell::Zsh => add_zsh_note_id_completion(&mut script),
    }

    script
}

fn add_zsh_note_id_completion(script: &mut String) {
    const INVOCATION: &str = "\nif [ \"$funcstack[1]\" = \"_nt\" ]; then";

    for (placeholder, completion) in [
        (":id:_default", ":id:_nt_note_ids"),
        ("*::ids:_default", "*::ids:_nt_note_ids"),
    ] {
        if !script.contains(placeholder) {
            panic!("clap_complete zsh output missing expected placeholder {placeholder:?}");
        }
        *script = script.replace(placeholder, completion);
    }

    let Some(index) = script.find(INVOCATION) else {
        panic!("clap_complete zsh output missing `_nt` dispatch marker");
    };
    script.insert_str(index, ZSH_NOTE_ID_COMPLETION);
}

const BASH_NOTE_ID_COMPLETION: &str = include_str!("completion_bash.sh");
const ZSH_NOTE_ID_COMPLETION: &str = include_str!("completion_zsh.sh");

#[cfg(test)]
mod tests {
    use crate::cli::Shell;

    use super::completion_script;

    #[test]
    fn bash_completion_contains_commands_and_dynamic_note_ids() {
        let script = completion_script(Shell::Bash);

        assert!(script.contains("init note todo list find show open"));
        assert!(script.contains("command nt list id 2>/dev/null"));
        assert!(script.contains("show:2|open:2"));
        assert!(script.contains("rm:*|update:2"));
        assert!(script.contains("export:[3-9]|export:[1-9][0-9]*"));
    }

    #[test]
    fn zsh_completion_contains_commands_and_dynamic_note_ids() {
        let script = completion_script(Shell::Zsh);

        assert!(script.contains("'show:'"));
        assert!(script.contains("'open:'"));
        assert!(script.contains(":id:_nt_note_ids"));
        assert!(script.contains("*::ids:_nt_note_ids"));
        assert!(script.contains("command nt list id 2>/dev/null"));

        let helper = script.find("_nt_note_ids()").unwrap();
        let invocation = script.find("_nt \"$@\"").unwrap();
        assert!(helper < invocation);
    }
}
