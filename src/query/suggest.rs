pub(super) fn edit_distance(left: &str, right: &str) -> usize {
    let mut previous: Vec<usize> = (0..=right.len()).collect();
    let mut current = vec![0; right.len() + 1];

    for (left_index, left_byte) in left.bytes().enumerate() {
        current[0] = left_index + 1;

        for (right_index, right_byte) in right.bytes().enumerate() {
            let replace = previous[right_index] + usize::from(left_byte != right_byte);
            let insert = current[right_index] + 1;
            let delete = previous[right_index + 1] + 1;
            current[right_index + 1] = replace.min(insert).min(delete);
        }

        previous.clone_from(&current);
    }

    previous[right.len()]
}

pub(super) fn query_field_suggestion(field: &str) -> Option<&'static str> {
    super::QUERY_FIELDS
        .iter()
        .copied()
        .map(|known| (edit_distance(field, known), known))
        .filter(|(distance, _)| *distance <= 2)
        .min_by_key(|(distance, known)| (*distance, known.len()))
        .map(|(_, known)| known)
}
