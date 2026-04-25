# Problems

## P1: README verifier looked for an unformatted `dbPath` phrase

The first verification pass failed because `verify-readme.ts` searched for
`dbPath is required`, while the README correctly documents the option as
`` `dbPath` is required ``. The implementation artifact was acceptable; the
verifier was too literal.

## Fix

Update `verify-readme.ts` to search for the formatted phrase used by the
README.
