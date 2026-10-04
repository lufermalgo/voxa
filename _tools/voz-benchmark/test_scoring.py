import unittest
from benchmark import errors, words, p95

class ScoringTests(unittest.TestCase):
    def test_substitute_insert_delete(self):
        self.assertEqual(errors("eins zwei drei", "eins vier drei"), (1, 3))
        self.assertEqual(errors("eins zwei", "eins und zwei"), (1, 2))
        self.assertEqual(errors("eins zwei drei", "eins drei"), (1, 3))

    def test_german_unicode_and_punctuation(self):
        self.assertEqual(errors("Grüße, Jörg!", "GRÜSSE jo\u0308rg"), (0, 2))
        self.assertEqual(words("Voxa_API 42"), ["voxa", "api", "42"])

    def test_silence_and_numeric_policy(self):
        self.assertEqual(errors("eins", ""), (1, 1))
        self.assertEqual(errors("zwei", "2"), (1, 1))

    def test_p95_nearest_rank(self):
        self.assertEqual(p95(list(range(1, 21))), 19)

if __name__ == "__main__":
    unittest.main()
