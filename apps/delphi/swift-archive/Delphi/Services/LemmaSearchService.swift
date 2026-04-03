import Foundation
import NaturalLanguage

/// In-memory lemma-based search using NLTagger.
/// Lemmatizes both indexed text and queries for morphological matching.
/// "купил" query matches "купить молоко" task, "задачами" matches "задача".
enum LemmaSearchService {

    // MARK: - Lemmatization

    /// Extract lemmas from text. Returns lowercased lemma set.
    static func lemmatize(_ text: String) -> Set<String> {
        let tagger = NLTagger(tagSchemes: [.lemma])
        tagger.string = text
        var lemmas = Set<String>()

        tagger.enumerateTags(in: text.startIndex..<text.endIndex, unit: .word, scheme: .lemma) { tag, range in
            if let lemma = tag?.rawValue {
                lemmas.insert(lemma.lowercased())
            } else {
                // Fallback: use the word itself if no lemma found
                let word = String(text[range]).lowercased().trimmingCharacters(in: .punctuationCharacters)
                if !word.isEmpty {
                    lemmas.insert(word)
                }
            }
            return true
        }

        return lemmas
    }

    /// Check if `text` matches `query` via lemma overlap.
    /// Returns true if ALL query lemmas are found in text lemmas.
    static func matches(text: String, query: String) -> Bool {
        let queryLemmas = lemmatize(query)
        guard !queryLemmas.isEmpty else { return false }
        let textLemmas = lemmatize(text)
        return queryLemmas.isSubset(of: textLemmas)
    }

    /// Score how well `text` matches `query`. Higher = better match.
    /// 0 = no match, 1.0 = all query lemmas found.
    /// Bonus for exact substring match.
    static func score(text: String, query: String) -> Double {
        let queryLower = query.lowercased()
        let textLower = text.lowercased()

        // Exact substring match — highest score
        if textLower.contains(queryLower) {
            // Boost: starts with query = even better
            if textLower.hasPrefix(queryLower) { return 1.5 }
            return 1.2
        }

        // Lemma match
        let queryLemmas = lemmatize(query)
        guard !queryLemmas.isEmpty else { return 0 }
        let textLemmas = lemmatize(text)

        let overlap = queryLemmas.intersection(textLemmas).count
        return Double(overlap) / Double(queryLemmas.count)
    }
}
