from pathlib import Path

path = Path("scripts/scheduler.lua")
content = path.read_text(encoding="utf-8")
replacements = {
    "if ctx.options.runs and not ctx.options.repeat then": (
        'if ctx.options.runs and not ctx.options["repeat"] then'
    ),
    "                repeat = ctx.options.repeat,": (
        '                ["repeat"] = ctx.options["repeat"],'
    ),
}

for old, new in replacements.items():
    if content.count(old) != 1:
        raise SystemExit(
            f"expected exactly one Lua keyword occurrence, found {content.count(old)}: {old!r}"
        )
    content = content.replace(old, new, 1)

path.write_text(content, encoding="utf-8")
