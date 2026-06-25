# Desloppify Evidence Sanity

## Trigger

After a desloppify cleanup slice writes baseline, after, or evidence files.

## Symptom

Evidence totals do not match the scan files, baseline and after look reversed, or
a subagent reports totals that do not agree.

## Do This

Parse the baseline and after JSON with Node, compute score, findings, and
high/medium/low plus target-rule counts, then update evidence from the parsed
JSON. Keep scan file order as baseline first, after second, and verify the
numbers before moving to the next slice.

```powershell
rtk node -e "const fs=require('fs'); const [base,after]=process.argv.slice(1).map(p=>JSON.parse(fs.readFileSync(p,'utf8'))); console.log({baseline:base.score, after:after.score, baselineFindings:base.findings?.length, afterFindings:after.findings?.length});" .tmp\baseline.json .tmp\after.json
```

## Avoid

Do not copy totals manually from chat. Do not overwrite baseline with after
scan. Do not continue to the next slice with inconsistent evidence.

## Promote To Skill When

This pattern repeats across more than one desloppify cleanup run.
