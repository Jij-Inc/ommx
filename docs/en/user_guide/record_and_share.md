# Save and share problems, results, and trials

**Choose what to save from the questions you want to answer later.** Candidate values alone do not identify the problem that was evaluated. You also do not need experiment management for every task. OMMX lets you exchange problems and results, store related data, and record multiple trials as needed.

This chapter is for readers handing existing problems or results to another person or process, or investigating past trials. You do not need to read the solving or transformation chapters first. Executable storage and retrieval examples belong to the API Reference.

## Choose by the recipient and the relationships you need

| Purpose | Choose | What to check |
|---|---|---|
| Pass the current model to an external tool that reads MPS | An MPS file | Supported expressions, variable kinds, and coefficient conventions. Not all OMMX labels and result-evaluation information survive the export |
| Pass problems, evaluated results, tables, and explanations together | An Artifact | Explicitly include the data and relationships that the recipient will need |
| Investigate conditions and results across several trials | An Experiment | Decide which inputs are shared and which conditions and results belong to individual trials |

Artifact provides storage for related data; Experiment records groups of trials on top of that storage. File exchange does not require a trial hierarchy. Neither is required for describing or transforming problems or evaluating candidates.

For example, you can send MPS to an external solver while keeping the original OMMX problem and the returned candidate in an Artifact. Passing a model to a tool and preserving information for interpreting its results serve different purposes. Reading MPS back does not necessarily recover the original labels or evaluation information.

## Start from the questions you will ask later

| Question | Examples of information to retain |
|---|---|
| Does this candidate meet the requirements? | The problem being evaluated, the candidate or evaluated result, and labels or reporting metrics that explain its application meaning |
| Can these two results be compared on the same basis? | Shared input, the objective and conditions being evaluated, trial settings, and their association with results |
| Why was this transformation or configuration chosen? | The original problem, the transformed problem when needed, the chosen policy or weights, and the reason for the choice |
| Can the calculation be run again? | Input data plus the code revision, dependency and solver versions, settings, random inputs, and other requirements for rerunning it |

For example, when comparing candidates obtained with different weights on penalties for constraint violations, distinguish values of the transformed objective from evaluations against the original problem. Keeping the problem that defines the comparison, alongside the weights and results, lets a later reader check that distinction. [Evaluate candidates and interpret results](evaluate_and_interpret.md) discusses comparison and acceptance decisions.

Experiment can record shared inputs and explanations, with conditions and data for each trial, called a Run. Logging an Adapter call associates the supplied problem and options with its returned result. It does not automatically collect every internally transformed problem, implicit setting, working Python variable, or detail of the execution environment. Record the information you will need explicitly.

A trial ending normally is also different from obtaining a feasible candidate or an optimal solution. Consult the saved evaluations and the backend's reports before accepting a result; the record's completion status alone does not establish either property.

## Make the data accessible to the recipient

For later use on the same machine, OMMX provides a local storage area called the Local Registry. To hand data to another person or machine, write an Artifact or Experiment to a file or distribute it through a remote registry. Finalizing a local record does not itself transfer a file or send data remotely.

A recipient can import the file into their own Local Registry and retrieve its saved data. They do not need the sender's Python objects or local storage. External data or code that was not included in the file is not transferred automatically.

Names and tags help readers find records, but can later point to different content. To identify exactly what was inspected, also record a digest: an identifier calculated from the content. Putting a problem and a result in the same package does not establish their relationship by itself; record which problem the result evaluates.

## Separate reading, continuing a record, and rerunning a calculation

Reading a saved result does not rerun the calculation. Adding new trials based on a committed Experiment, or restoring an interrupted record from a checkpoint, also does not resume a solver's internal search state.

Rerunning requires the relevant code, data, and execution environment to remain available. Even when these are recorded, external services, randomness, or implementation differences can affect the result. Decide whether the goal is to retrieve results, inspect their conditions, or execute the calculation again.

## Continue to complete procedures

| Next task | Explanation and examples |
|---|---|
| Save problems and results and retrieve them in another process | {class}`~ommx.artifact.Artifact` |
| Record trial conditions and results and read them later | {class}`~ommx.experiment.Experiment` |
| Add trials to a saved record or restore a checkpoint | {meth}`~ommx.experiment.Experiment.fork`, {meth}`~ommx.experiment.Experiment.restore_from_checkpoint` |
| Export MPS for another tool | {meth}`~ommx.Instance.save_mps` |
| Find saved data and manage local storage | [Local Registry operations](../how_to/manage_local_registry.md) |
| Distribute data through a remote registry | {meth}`~ommx.artifact.Artifact.push` and the [registry authentication reference](../how_to/share_artifact_registry.md) |

For tools that read or write the formats directly, consult the [Message definitions](https://github.com/Jij-Inc/ommx/tree/main/proto) and [Artifact format specification](https://github.com/Jij-Inc/ommx/blob/main/rust/ommx/doc/artifact_design.md). SDK versions and the versions of stored formats or artifact kinds are distinct; check the recipient's supported formats as well.
