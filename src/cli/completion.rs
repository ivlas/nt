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

const BASH_NOTE_ID_COMPLETION: &str = include_str!("completion_bash.sh");

const ZSH_NOTE_ID_COMPLETION: &str = include_str!("completion_zsh.sh");

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
        assert!(script.contains("links) _nt_complete_link_filter"));
        assert!(script.contains("compgen -W \"from: to:\""));
        assert!(script.contains("from|to) _nt_complete_prefixed_values"));
        assert!(!script.contains("compgen -W \"from to\""));
        assert!(script.contains("export:[3-9]|export:[1-9][0-9]*"));
        assert!(script.contains("rm:*|update:2"));
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
        assert!(script.contains("_nt_link_filter_arg()"));
        assert!(script.contains("fields+=(from: to:)"));
        assert!(script.contains("from|to) _nt_complete_prefixed_values"));
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
