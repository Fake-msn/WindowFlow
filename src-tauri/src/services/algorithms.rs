use std::collections::{HashMap, HashSet};

// ============================================================
// [T3-1] 时间衰减权重
// ============================================================

/// 时间衰减权重：距今 `age_hours` 小时的事件权重。
/// 每经过 `half_life_hours`，权重衰减一半。返回 (0, 1]。
pub fn time_decay_weight(age_hours: f64, half_life_hours: f64) -> f64 {
    if half_life_hours <= 0.0 {
        return 1.0;
    }
    0.5f64.powf(age_hours.max(0.0) / half_life_hours)
}

// ============================================================
// [T3-2] Apriori 频繁项集挖掘
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub struct FrequentItemset {
    /// 项集内容（字典序排序）
    pub items: Vec<String>,
    /// 支持度（包含该项集的事务数）
    pub support: usize,
}

/// Apriori 频繁项集挖掘。
/// - `transactions`: 每个事务是一次会话中共同出现的应用集合
/// - `min_support`: 最小支持度阈值
///
/// 返回长度 >= 2 的频繁项集，按支持度降序。
pub fn apriori(transactions: &[Vec<String>], min_support: usize) -> Vec<FrequentItemset> {
    if transactions.is_empty() || min_support == 0 {
        return Vec::new();
    }

    // 规范化事务：去重 + 排序
    let tx: Vec<Vec<String>> = transactions
        .iter()
        .map(|t| {
            let mut s: Vec<String> = t.iter().cloned().collect::<HashSet<_>>().into_iter().collect();
            s.sort();
            s
        })
        .collect();

    // L1：频繁 1-项集
    let mut counts: HashMap<Vec<String>, usize> = HashMap::new();
    for t in &tx {
        for item in t {
            *counts.entry(vec![item.clone()]).or_insert(0) += 1;
        }
    }
    let mut current: Vec<Vec<String>> = counts
        .iter()
        .filter(|(_, &c)| c >= min_support)
        .map(|(k, _)| k.clone())
        .collect();
    current.sort();

    let mut result: Vec<FrequentItemset> = Vec::new();
    let mut k = 2usize;

    // 逐层生成 Lk（限制最大长度 5，避免组合爆炸）
    while !current.is_empty() && k <= 5 {
        let candidates = generate_candidates(&current, k);
        let mut cand_counts: HashMap<Vec<String>, usize> = HashMap::new();

        for t in &tx {
            let tset: HashSet<&String> = t.iter().collect();
            for cand in &candidates {
                if cand.iter().all(|it| tset.contains(it)) {
                    *cand_counts.entry(cand.clone()).or_insert(0) += 1;
                }
            }
        }

        let mut frequent: Vec<Vec<String>> = Vec::new();
        for (itemset, count) in &cand_counts {
            if *count >= min_support {
                frequent.push(itemset.clone());
                result.push(FrequentItemset {
                    items: itemset.clone(),
                    support: *count,
                });
            }
        }
        frequent.sort();
        current = frequent;
        k += 1;
    }

    // 支持度降序，其次项集越大越靠前
    result.sort_by(|a, b| {
        b.support
            .cmp(&a.support)
            .then(b.items.len().cmp(&a.items.len()))
            .then(a.items.cmp(&b.items))
    });
    result
}

/// Apriori-gen：由频繁 (k-1)-项集连接生成候选 k-项集。
/// 两个 (k-1)-项集共享前 k-2 项时可连接。
fn generate_candidates(prev: &[Vec<String>], k: usize) -> Vec<Vec<String>> {
    let mut cands: HashSet<Vec<String>> = HashSet::new();
    for i in 0..prev.len() {
        for j in (i + 1)..prev.len() {
            let a = &prev[i];
            let b = &prev[j];
            if a.len() == k - 1 && b.len() == k - 1 && a[..k - 2] == b[..k - 2] {
                let mut merged = a.clone();
                merged.push(b[k - 2].clone());
                merged.sort();
                merged.dedup();
                if merged.len() == k {
                    cands.insert(merged);
                }
            }
        }
    }
    cands.into_iter().collect()
}

// ============================================================
// [T3-3] PrefixSpan 序列模式挖掘
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub struct SequentialPattern {
    /// 有序模式（应用切换序列）
    pub pattern: Vec<String>,
    /// 支持度（包含该子序列的序列数）
    pub support: usize,
}

/// PrefixSpan 序列模式挖掘（单项事件版本）。
/// - `sequences`: 每个序列是有序的应用切换记录
/// - `min_support`: 最小支持度
/// - `max_len`: 最大模式长度
///
/// 返回频繁序列模式（含长度 1），按支持度降序。
pub fn prefixspan(sequences: &[Vec<String>], min_support: usize, max_len: usize) -> Vec<SequentialPattern> {
    if sequences.is_empty() || min_support == 0 || max_len == 0 {
        return Vec::new();
    }
    let proj: Vec<&[String]> = sequences.iter().map(|s| s.as_slice()).collect();
    let mut out: Vec<SequentialPattern> = Vec::new();
    prefixspan_mine(&[], &proj, min_support, max_len, &mut out);
    out.sort_by(|a, b| {
        b.support
            .cmp(&a.support)
            .then(b.pattern.len().cmp(&a.pattern.len()))
            .then(a.pattern.cmp(&b.pattern))
    });
    out
}

fn prefixspan_mine(
    prefix: &[String],
    projected: &[&[String]],
    min_support: usize,
    max_len: usize,
    out: &mut Vec<SequentialPattern>,
) {
    if prefix.len() >= max_len {
        return;
    }
    // 统计投影数据库中每个项的支持度（按序列计一次）
    let mut counts: HashMap<String, usize> = HashMap::new();
    for suffix in projected {
        let seen: HashSet<&String> = suffix.iter().collect();
        for it in seen {
            *counts.entry(it.clone()).or_insert(0) += 1;
        }
    }
    let mut items: Vec<(String, usize)> = counts.into_iter().filter(|(_, c)| *c >= min_support).collect();
    items.sort_by(|a, b| a.0.cmp(&b.0));

    for (item, support) in items {
        let mut new_prefix = prefix.to_vec();
        new_prefix.push(item.clone());
        out.push(SequentialPattern {
            pattern: new_prefix.clone(),
            support,
        });

        // 投影：每个后缀取 item 首次出现之后的部分
        let new_proj: Vec<&[String]> = projected
            .iter()
            .filter_map(|s| s.iter().position(|x| x == &item).map(|pos| &s[pos + 1..]))
            .collect();

        prefixspan_mine(&new_prefix, &new_proj, min_support, max_len, out);
    }
}

/// 基于序列模式预测：给定 `current` 应用，返回最可能紧随其后的应用及支持度，按支持度降序。
pub fn predict_next(patterns: &[SequentialPattern], current: &str) -> Vec<(String, usize)> {
    let mut next: Vec<(String, usize)> = patterns
        .iter()
        .filter(|p| p.pattern.len() == 2 && p.pattern[0] == current)
        .map(|p| (p.pattern[1].clone(), p.support))
        .collect();
    next.sort_by(|a, b| b.1.cmp(&a.1));
    next
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_time_decay_weight() {
        assert!((time_decay_weight(0.0, 24.0) - 1.0).abs() < 1e-9);
        assert!((time_decay_weight(24.0, 24.0) - 0.5).abs() < 1e-9);
        assert!((time_decay_weight(48.0, 24.0) - 0.25).abs() < 1e-9);
        assert!((time_decay_weight(100.0, 0.0) - 1.0).abs() < 1e-9);
        assert!(time_decay_weight(10.0, 24.0) > time_decay_weight(20.0, 24.0));
    }

    #[test]
    fn test_apriori_basic() {
        let transactions = vec![
            vec!["code".to_string(), "chrome".to_string(), "terminal".to_string()],
            vec!["code".to_string(), "chrome".to_string()],
            vec!["code".to_string(), "chrome".to_string(), "terminal".to_string()],
            vec!["word".to_string(), "chrome".to_string()],
        ];
        let itemsets = apriori(&transactions, 2);
        let top = &itemsets[0];
        assert_eq!(top.items, vec!["chrome".to_string(), "code".to_string()]);
        assert_eq!(top.support, 3);
        let triple = itemsets.iter().find(|s| s.items.len() == 3);
        assert!(triple.is_some());
        assert_eq!(triple.unwrap().support, 2);
    }

    #[test]
    fn test_apriori_min_support_filter() {
        let transactions = vec![
            vec!["a".to_string(), "b".to_string()],
            vec!["a".to_string(), "c".to_string()],
        ];
        assert!(apriori(&transactions, 2).is_empty());
        assert!(!apriori(&transactions, 1).is_empty());
    }

    #[test]
    fn test_apriori_empty() {
        assert!(apriori(&[], 2).is_empty());
        assert!(apriori(&[vec!["a".to_string()]], 0).is_empty());
    }

    #[test]
    fn test_prefixspan_basic() {
        let sequences = vec![
            vec!["a".to_string(), "b".to_string(), "c".to_string()],
            vec!["a".to_string(), "b".to_string(), "d".to_string()],
            vec!["a".to_string(), "b".to_string(), "c".to_string()],
        ];
        let patterns = prefixspan(&sequences, 2, 3);
        let ab = patterns.iter().find(|p| p.pattern == vec!["a".to_string(), "b".to_string()]);
        assert!(ab.is_some());
        assert_eq!(ab.unwrap().support, 3);
        let abc = patterns
            .iter()
            .find(|p| p.pattern == vec!["a".to_string(), "b".to_string(), "c".to_string()]);
        assert!(abc.is_some());
        assert_eq!(abc.unwrap().support, 2);
    }

    #[test]
    fn test_prefixspan_max_len() {
        let sequences = vec![
            vec!["a".to_string(), "b".to_string(), "c".to_string(), "d".to_string()],
            vec!["a".to_string(), "b".to_string(), "c".to_string(), "d".to_string()],
        ];
        let patterns = prefixspan(&sequences, 2, 2);
        assert!(patterns.iter().all(|p| p.pattern.len() <= 2));
    }

    #[test]
    fn test_predict_next() {
        let sequences = vec![
            vec!["editor".to_string(), "browser".to_string()],
            vec!["editor".to_string(), "browser".to_string()],
            vec!["editor".to_string(), "terminal".to_string()],
        ];
        let patterns = prefixspan(&sequences, 2, 2);
        let preds = predict_next(&patterns, "editor");
        assert_eq!(preds[0].0, "browser");
        assert_eq!(preds[0].1, 2);
    }
}
