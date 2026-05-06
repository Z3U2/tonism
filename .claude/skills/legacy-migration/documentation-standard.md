# AI Mod Project Documentation & Conventions

# Intent

---

Enable AI agents and developers to quickly understand and consistently contribute to a codebase through effective context priming and standardized conventions.

## Key Points

---

### **Document only project-specific decisions** – Focus on the non-universal parts of your codebase: custom conventions, architectural choices, and patterns that deviate from or extend framework defaults.

_AI models are already trained on framework documentation and common patterns. Restating this information wastes context window space and adds noise. Project-specific decisions are what AI cannot infer and what new developers actually need to learn._

| ✅ Good                                                                                                                                            | ❌ Avoid                                                                                                                               |
| -------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------- |
| All API responses use our ApiResponse<T> wrapper with data, error, and metadata fields. Errors must include a code from /constants/error-codes.ts. | API endpoints should return appropriate HTTP status codes (200 for success, 404 for not found). Use try-catch blocks to handle errors. |

### **Use universal language and existing abstractions** – Describe architecture and standards using well-known patterns and terminology (e.g., "repository pattern," "service layer") to be concise and leverage shared understanding.

_Named patterns carry dense meaning. Saying "repository pattern" instantly conveys structure, responsibilities, and interfaces without lengthy explanation. AI models recognize these patterns and can apply them correctly. Custom explanations of the same concepts introduce ambiguity and inconsistency._

| ✅ Good                                                                                                                                                                                                                                                                                                            | ❌ Avoid                                                                                                                                                                                                                                                                                                                                                                                                                          |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| The backend follows hexagonal architecture with a functional core / imperative shell split. Domain logic is pure and side-effect free; adapters handle I/O at the boundaries. Services use railway programming for error handling—operations return Result<T, E> and are composed using map, flatMap, and recover. | Business logic is in service classes that don't call the database directly. Instead, they return objects that can either be a success or a failure. The success case contains the data, the failure case contains an error. We chain these together so that if one step fails, the rest are skipped. External systems like databases and APIs are accessed through adapter classes that implement interfaces defined in the core. |
| State changes emit domain events (event-driven architecture). Consumers are decoupled and registered in /events/handlers.ts. Test coverage for domain logic uses boundary value analysis—test at edges and equivalence class boundaries.                                                                           | When something important happens, we publish an event object. Other parts of the system can listen for these events and react to them. This keeps things loosely coupled. For testing, make sure to test edge cases like zero, one, maximum values, and just above/below limits.                                                                                                                                                  |
| Domain modules follow functional programming principles: immutable data structures, pure functions, composition over inheritance. Side effects are pushed to the shell layer.                                                                                                                                      | Don't modify objects directly—create new copies with the changes. Functions should always return the same output for the same input and shouldn't change anything outside themselves. Build complex behavior by combining smaller functions rather than using class inheritance.                                                                                                                                                  |

### **Reference established literature** – When introducing patterns or methodologies, cite recognized authors and works to provide developers with authoritative sources for deeper understanding.

_AI models are trained on these books and can apply patterns more accurately when explicitly named. Citations also give developers a path to deeper learning for edge cases your documentation doesn't cover, and create a shared vocabulary across the organization._

| ✅ Good                                                                                                                                                                                                                | ❌ Avoid                                                                                                                                                                                                                                                                                             |
| ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Comments follow the principles from A Philosophy of Software Design (John Ousterhout): document what is not obvious from the code, focus on why over what, and write comments before implementation to clarify intent. | Write comments that explain why code exists, not what it does. Good comments describe intent, design decisions, and non-obvious behavior. Don't comment obvious things like // increment counter above counter++. Write your comments first to help clarify your thinking before you write the code. |
| The backend follows hexagonal architecture (Alistair Cockburn) with a functional core / imperative shell split (Gary Bernhardt). Domain logic is pure and side-effect free; adapters handle I/O at the boundaries.     |                                                                                                                                                                                                                                                                                                      |
| Error handling uses railway-oriented programming (Scott Wlaschin, Domain Modeling Made Functional). Operations return Result<T, E> and are composed using map, flatMap, and recover.                                   |                                                                                                                                                                                                                                                                                                      |
| Test design applies boundary value analysis (Glenford Myers, The Art of Software Testing) and equivalence partitioning for domain logic coverage.                                                                      |                                                                                                                                                                                                                                                                                                      |

### **Provide short, concrete examples** – Include brief code snippets that illustrate the expected implementation style; one clear example is worth more than lengthy explanations.

_Examples eliminate ambiguity. Written descriptions can be interpreted multiple ways; code cannot. AI models excel at pattern matching from examples and will replicate the structure, naming, and style they see. A 10-line example often communicates more than a page of explanation._

✅ Good

```tsx
// Adding a new domain service
export class OrderService extends BaseService {
  constructor(
    private orderRepo: OrderRepository,
    private inventory: InventoryService,
    eventBus: EventBus,
  ) {
    super(eventBus);
  }

  async create(dto: CreateOrderDto): Promise<Order> {
    await this.inventory.reserve(dto.items);
    const order = await this.orderRepo.save(Order.fromDto(dto));
    this.emit(new OrderCreatedEvent(order));
    return order;
  }
}
```

❌ Avoid

```tsx
*// This is how you create a service in our codebase// First, you need to create a class// Services should have a constructor that receives dependencies// Dependencies are injected automatically by our DI container// You can find the DI configuration in /src/di/container.ts// Make sure to register your service there after creating it// Services typically interact with repositories to access data// Here's an example of a simple service:*

import { Injectable } from '@nestjs/common';

@Injectable()
export class OrderService {
  *// ... full implementation with extensive comments*
}
```

### **Apply the "new developer" test** – Ask yourself: what specifics would a competent developer, already familiar with the tools and frameworks, need to know to implement a representative feature? Document only that.

_This mental filter prevents both over-documentation (explaining what developers already know) and under-documentation (assuming tribal knowledge). It focuses documentation on the gap between general expertise and your specific codebase—exactly what accelerates onboarding._

| ✅ Good | ❌ Avoid |
| ------- | -------- |

| _Example: Backend with authentication_

A new developer implementing an authenticated endpoint would need to know:

**Architecture overview** – Layer structure (controllers → services → repositories) with sequence diagram if flows are non-obvious

**Authentication & authorization** – How to protect routes, access current user, enforce permissions

**Transport standards** – DTO conventions, request/response shaping, OpenAPI documentation requirements

**Testing expectations** – Unit test patterns, e2e test setup, how to mock authenticated requests

**Logging** – What to log, log levels, how to access the logger

**Naming & style** – File naming, class naming, code formatting rules specific to the project

**Comments** – When and how to comment (or not)

**Common commands** – How to run, test, lint, and generate migrations | _Avoid documenting:_

How NestJS decorators work

What dependency injection is

How Jest matchers function

General REST API principles |

### **Split documentation by work type** – Organize into separate files by domain (backend, frontend, data, infrastructure) to enable granular context priming and avoid loading irrelevant information for a given task.

_Context windows are limited. Loading frontend documentation when working on backend code wastes tokens and can confuse AI with irrelevant patterns. Granular files let you prime the AI with exactly the context needed for each task, maximizing the useful work per session._

```markdown
/docs/ai-context
├── backend
│ ├── architecture.md # Layers, patterns, dependency injection, module structure
│ ├── style.md # Naming, file organization, code formatting conventions
│ └── testing.md # Test structure, mocking strategies, coverage expectations
└── frontend
├── architecture.md # Component hierarchy, state management, routing patterns
├── style.md # Component naming, CSS conventions, file colocation
└── testing.md # Component testing, mocking APIs, test utilities
```

**Separate documentation from AI prompts and AI specific files** – Keep all developer-relevant knowledge in the project documentation itself, not embedded in AI prompts.

- _Documentation_ describes architecture, patterns, and code standards
- _Prompt_ injects the relevant documentation and establishes the AI's role (e.g., a senior developer following project standards)

_Documentation embedded in prompts is invisible to human developers, creating a knowledge silo. It also becomes harder to maintain and version. Clean separation means one source of truth for both humans and AI, and prompts stay simple and reusable across different tasks._

| ✅ Good | ❌ Avoid |
| ------- | -------- |

| **Documentation (backend.md):**

Services emit domain events after state mutations. Extend BaseService and call this.emit() with the appropriate event class from /events.

**Prompt:**

You are a senior developer on this project. Use the attached documentation as your reference for architecture and standards. Implement the requested feature. | **Prompt containing everything:**

You are a senior developer. In this project, services must emit domain events after mutations. Always extend BaseService. Events are in the /events folder. Use this.emit() to dispatch them. Also, repositories extend BaseRepository. Register new services in /di/container.ts. For validation, use class-validator decorators... |

# Common errors

---

- **Exceeding 2000 lines** – This documentation serves as context priming; overly long documents consume context window space and limit the AI's ability to do meaningful work in a session.
- **Documenting framework defaults** – Avoid restating conventions and standards already inherent to the tools or frameworks being used; AI models are already trained on these.
- **Being too generic** – Focus on project-specific decisions and patterns, not general overviews of technologies that AI already understands.
- **Centralizing all documentation in a single file** – Avoid placing everything in one Agents.md or Claude.md file; different tasks require different context, and injecting irrelevant documentation diminishes effectiveness for developers working on other aspects of the project.
