use crate::types::{GroupedIndices, JumpTargetType};

pub(crate) fn find_required_slot_sizes(num_indices: usize, group_length: usize) -> Vec<usize> {
    if num_indices <= group_length {
        return vec![1; num_indices];
    }
    let mut slot_sizes = vec![1; group_length];
    let mut next_increase_slot = group_length - 1;
    while slot_sizes.iter().sum::<usize>() < num_indices {
        slot_sizes[next_increase_slot] *= group_length;
        next_increase_slot = (next_increase_slot + group_length - 1) % group_length;
        let previous = (next_increase_slot + 1) % group_length;
        let sum = slot_sizes.iter().sum::<usize>();
        if sum > num_indices {
            slot_sizes[previous] -= sum - num_indices;
        }
    }
    slot_sizes
}

pub(crate) fn group_indices(indices: &[usize], group_length: usize) -> Option<GroupedIndices> {
    if indices.is_empty() {
        return None;
    }
    if indices.len() == 1 {
        return Some(GroupedIndices::Leaf(indices[0]));
    }

    let slot_sizes = find_required_slot_sizes(indices.len(), group_length);
    let mut start = 0;
    let mut groups = Vec::with_capacity(slot_sizes.len());
    for slot_size in slot_sizes {
        let end = start + slot_size;
        let part = &indices[start..end];
        groups.push(group_indices(part, group_length)?);
        start = end;
    }
    Some(GroupedIndices::Group(groups))
}

fn collect_leaves(group_or_index: &GroupedIndices, leaves: &mut Vec<usize>) {
    match group_or_index {
        GroupedIndices::Leaf(index) => leaves.push(*index),
        GroupedIndices::Group(groups) => {
            for sub in groups {
                collect_leaves(sub, leaves);
            }
        }
    }
}

pub(crate) fn generate_jump_targets(
    grouped_indices: &GroupedIndices,
    target_keys: &[char],
) -> Vec<(JumpTargetType, usize, char)> {
    let mut out = Vec::new();
    let top_groups = match grouped_indices {
        GroupedIndices::Leaf(index) => {
            out.push((JumpTargetType::Direct, *index, target_keys[0]));
            return out;
        }
        GroupedIndices::Group(groups) => groups,
    };

    for (target_key, group_or_index) in target_keys.iter().zip(top_groups.iter()) {
        match group_or_index {
            GroupedIndices::Leaf(index) => out.push((JumpTargetType::Direct, *index, *target_key)),
            GroupedIndices::Group(subgroups) => {
                for (preview_key, sub_group_or_index) in target_keys.iter().zip(subgroups.iter()) {
                    let mut leaves = Vec::new();
                    collect_leaves(sub_group_or_index, &mut leaves);
                    for leave in leaves {
                        out.push((JumpTargetType::Group, leave, *target_key));
                        out.push((JumpTargetType::Preview, leave + 1, *preview_key));
                    }
                }
            }
        }
    }
    out
}
