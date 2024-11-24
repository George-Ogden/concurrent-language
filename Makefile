parse:
	cat sample.txt | antlr4-parse Grammar.g4  program -tree

.PHONY: parse
