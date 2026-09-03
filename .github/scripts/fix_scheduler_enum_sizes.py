from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    file = Path(path)
    content = file.read_text(encoding="utf-8")
    if content.count(old) != 1:
        raise SystemExit(
            f"expected exactly one occurrence in {path}, found {content.count(old)}: {old!r}"
        )
    file.write_text(content.replace(old, new, 1), encoding="utf-8")


replace_once(
    "src/scheduler/model.rs",
    "    Created(ScheduledJob),\n",
    "    Created(Box<ScheduledJob>),\n",
)
replace_once(
    "src/scheduler/model.rs",
    "    Updated(ScheduledJob),\n",
    "    Updated(Box<ScheduledJob>),\n",
)
replace_once(
    "src/scheduler/memory.rs",
    "        CreateJobOutcome::Created(job)\n",
    "        CreateJobOutcome::Created(Box::new(job))\n",
)
replace_once(
    "src/scheduler/memory.rs",
    "        JobMutationOutcome::Updated(job.clone())\n",
    "        JobMutationOutcome::Updated(Box::new(job.clone()))\n",
)
replace_once(
    "src/scheduler/postgres.rs",
    "        Ok(CreateJobOutcome::Created(created))\n",
    "        Ok(CreateJobOutcome::Created(Box::new(created)))\n",
)
replace_once(
    "src/scheduler/postgres.rs",
    "        Ok(JobMutationOutcome::Updated(updated))\n",
    "        Ok(JobMutationOutcome::Updated(Box::new(updated)))\n",
)
replace_once(
    "tests/scheduler.rs",
    "        CreateJobOutcome::Created(job) => job,\n",
    "        CreateJobOutcome::Created(job) => *job,\n",
)
