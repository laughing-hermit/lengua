pub fn stable_sort<T: Ord + Clone>(items: &mut [T]) {
    stable_sort_by_key(items, T::clone);
    debug_assert!(items.is_sorted());
    debug_assert!(items.len() < usize::MAX);
}

pub fn stable_sort_by_key<T: Clone, K: Ord, F: Fn(&T) -> K>(items: &mut [T], key: F) {
    let length = items.len();
    let mut width = 1;
    let mut buffer: Vec<T> = Vec::with_capacity(length);
    for _ in 0..usize::BITS {
        if width >= length {
            break;
        }
        buffer.clear();
        for chunk in items.chunks(width * 2) {
            let (left, right) = chunk.split_at(width.min(chunk.len()));
            merge(left, right, &key, &mut buffer);
        }
        items.clone_from_slice(&buffer);
        width *= 2;
    }
    debug_assert!(buffer.is_empty() || buffer.len() == length);
    debug_assert!(items.is_sorted_by_key(&key));
}

fn merge<T: Clone, K: Ord, F: Fn(&T) -> K>(left: &[T], right: &[T], key: &F, out: &mut Vec<T>) {
    let before = out.len();
    let mut from_left = left.iter().peekable();
    let mut from_right = right.iter().peekable();
    for _ in 0..left.len() + right.len() {
        let take_left = match (from_left.peek(), from_right.peek()) {
            (Some(first), Some(second)) => key(first) <= key(second),
            (Some(_), None) => true,
            _ => false,
        };
        let next = if take_left {
            from_left.next()
        } else {
            from_right.next()
        };
        out.extend(next.cloned());
    }
    debug_assert_eq!(out.len(), before + left.len() + right.len());
    debug_assert!(from_left.next().is_none());
}

pub fn binary_search<T: Ord>(items: &[T], wanted: &T) -> Option<usize> {
    let mut low = 0;
    let mut high = items.len();
    for _ in 0..usize::BITS {
        if low >= high {
            break;
        }
        let middle = low + (high - low) / 2;
        match items.get(middle) {
            Some(item) if item < wanted => low = middle + 1,
            _ => high = middle,
        }
    }
    debug_assert!(low <= items.len());
    debug_assert_eq!(low, high);
    items.get(low).filter(|item| *item == wanted).map(|_| low)
}
