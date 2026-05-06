# Writing an Architecture Decision Record (ADR)

Owner: Francois Hendriks
Maturity: Quality Standard
Tech Tag: Architecture

## What

An architecture decision record (ADR) is a document that captures an architecture decision made along with its context and consequences.

## Intent

The ADR serves multiple purposes:

- Help teams make architecture decision based on a given context and problem
- Share and communicate the decisions taken on a project
- Understanding and re-evaluating past decisions

## Key points

- An ADR should contain:

  - **The context**: What is the problem we are trying to solve by taking the decision? What are the impacts of that decision?
    - **The date** at which the decision has been taken: This is especially useful to understand the context in which the decision was made.
    - **The owner** of the ADR
  - **The decision criteria/drivers**: On which criteria is the decision going to be based? The criteria need to be based on the needs of the team or client.
  - **The identified solutions**: All the solutions that could be used to solve the problem at hand
  - **The decision matrix**: A matrix evaluating each solution based on a criterium defined above
  - **The final decision**

- The solutions should be compared relatively to one another for each criterium. Use specific icons or symbols to identify which one is better.
- The best solutions should be easily identifiable

## Common pitfalls

- **ADR to Compare two libraries between them**

> _Ex: MaterialUI vs MineralUI_

An ADR is related to a topic. An ADR is not done to compare two libraries but for understanding how we respond to a problem according to the customer context. Choosing to consider only two libraries is the result of an analysis that should be documented.

- Not adding the current adopted solution to the list of choices
- Multiple problems in the same ADR
- Copying and reusing an existing ADR from another project

## Good articles to read

- [How ADRs are made elsewhere (Octo)](https://blog.octo.com/architecture-decision-record)
- [Tips and comments about ADR](https://github.com/joelparkerhenderson/architecture-decision-record#suggestions-for-writing-good-adrs)
