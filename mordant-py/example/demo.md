---
title: Mordant Viewer Feature Demo
author: The Mordant Team
date: 2026-07-12
tags: [mordant, markdown, demo, mermaid, katex]
description: A showcase of every rendering feature supported by the mordant md_viewer.
---

# 🚀 Mordant Viewer Feature Demo

Welcome to a **single document** that exercises _almost every_ feature of the
Rust-powered `mordant` renderer. Drag this file into the viewer (or use the 📄
button) and toggle the **Sync mermaid** checkbox to recolor all diagrams with
your code-highlighting theme :sparkles:.

> 💡 Tip: change the *Highlighting* dropdown and watch both code blocks and
> Mermaid diagrams restyle together when **Sync mermaid** is enabled.

This paragraph demonstrates inline styles: **bold**, *italic*, ***both***,
`inline code`, ~~strikethrough~~, and a [link to the project](https://github.com).
Here is an inline formula: $E = mc^2$, and a second one with a square root:
$\sqrt{x^2 + y^2}$.

---

## 1. Code highlighting (5 languages)

### Python :snake:

```python
from dataclasses import dataclass


@dataclass
class Point:
    x: float
    y: float

    def distance(self, other: "Point") -> float:
        return ((self.x - other.x) ** 2 + (self.y - other.y) ** 2) ** 0.5


if __name__ == "__main__":
    p = Point(0.0, 0.0)
    q = Point(3.0, 4.0)
    print(f"distance = {p.distance(q)}")  # 5.0
```

### Rust :crab:

```rust
use std::collections::HashMap;

fn word_count(text: &str) -> HashMap<&str, usize> {
    let mut counts = HashMap::new();
    for word in text.split_whitespace() {
        *counts.entry(word).or_insert(0) += 1;
    }
    counts
}

fn main() {
    let map = word_count("the quick brown fox the lazy dog");
    println!("the -> {}", map["the"]);
}
```

### TypeScript :computer:

```typescript
interface User {
  id: number;
  name: string;
  roles: string[];
}

async function fetchUser(id: number): Promise<User> {
  const res = await fetch(`/api/users/${id}`);
  if (!res.ok) throw new Error(`HTTP ${res.status}`);
  return (await res.json()) as User;
}

fetchUser(1).then((u) => console.log(u.name));
```

### Go :globe_with_meridians:

```go
package main

import (
	"fmt"
	"sort"
)

func main() {
	nums := []int{5, 2, 9, 1, 5, 6}
	sort.Ints(nums)
	fmt.Println("sorted:", nums)

	total := 0
	for _, n := range nums {
		total += n
	}
	fmt.Println("sum:", total)
}
```

### C++ :gear:

```cpp
#include <iostream>
#include <vector>
#include <algorithm>

int main() {
    std::vector<int> v{5, 2, 9, 1, 5, 6};
    std::sort(v.begin(), v.end());
    for (int n : v) std::cout << n << ' ';
    std::cout << '\n';
    return 0;
}
```

---

## 2. Tables, lists & blockquotes

### GFM table with alignment

| Language | Year | Typed? | Emoji |
|:---------|-----:|:------:|:-----:|
| Python   | 1991 | dynamic | :snake: |
| Rust     | 2010 | static  | :crab: |
| Go       | 2009 | static  | :globe_with_meridians: |
| TypeScript | 2012 | static | :computer: |
| C++      | 1985 | static  | :gear: |

### Task list

- [x] Parse markdown with Rushdown
- [x] Highlight code with syntect themes
- [ ] Conquer the universe :rocket:
- [ ] Write more docs

### Ordered & nested list

1. Load the document
2. Parse to an AST
   1. Extract headings, tables, diagrams
   2. Resolve emoji shortcodes
3. Render to HTML

### Blockquote

> "Documentation is a love letter that you write to your future self."
>
> — Damian Conway

---

## 3. Mermaid diagrams

> The viewer renders these **server-side** as inline SVG by default. With
> **Sync mermaid** on, the colors come from your code-highlighting theme.

### 3.1 Flowchart

```mermaid
graph TD
    A[Start] --> B{Is it working?}
    B -- Yes --> C[Ship it! 🚀]
    B -- No --> D[Debug]
    D --> B
    C --> E([Done])
```

### 3.2 Sequence diagram

```mermaid
sequenceDiagram
    participant U as User
    participant V as Viewer
    participant M as Mordant
    U->>V: Open demo.md
    V->>M: markdown_to_html(src, gfm)
    M-->>V: HTML + SVG
    V-->>U: Render in WebView
```

### 3.3 Class diagram

```mermaid
classDiagram
    class Document {
        +children: Node[]
        +metadata: dict
        +walk(mode)
    }
    class Node {
        +kind: str
        +text: str
        +children: Node[]
    }
    Document "1" *-- "many" Node : contains
```

### 3.4 State diagram

```mermaid
stateDiagram-v2
    [*] --> Idle
    Idle --> Parsing: load
    Parsing --> Rendering: ok
    Rendering --> Done: svg
    Done --> Idle: reset
    Rendering --> Error: fail
    Error --> Idle: retry
```

### 3.5 Entity-relationship

```mermaid
erDiagram
    CUSTOMER ||--o{ ORDER : places
    ORDER ||--|{ LINE_ITEM : contains
    PRODUCT ||--o{ LINE_ITEM : "sold in"
    CUSTOMER {
        string name
        string email
    }
    ORDER {
        int order_id
        datetime created_at
    }
```

### 3.6 Gantt chart

```mermaid
gantt
    title Mordant 0.9 Roadmap
    dateFormat YYYY-MM-DD
    section Core
    Parser polish      :a1, 2026-08-01, 14d
    Renderer cache     :a2, after a1, 10d
    section Docs
    Rewrite README     :b1, 2026-08-03, 7d
    Video tutorial     :b2, after b1, 5d
```

### 3.7 Pie chart

```mermaid
pie title What we ship
    "Rust code" : 55
    "Python bindings" : 25
    "Docs" : 12
    "Tests" : 8
```

### 3.8 Journey

```mermaid
journey
    title Onboarding a new user
    section Discover
      Visit site: 5: User
      Read docs: 4: User
    section Build
      cargo build: 3: User
      Run viewer: 5: User
```

### 3.9 Git graph

```mermaid
gitGraph
    commit
    commit
    branch feature
    checkout feature
    commit
    commit
    checkout main
    merge feature
    commit
```

### 3.10 Mindmap

```mermaid
mindmap
  root((Mordant))
    Parsing
      Rushdown
      AST
    Rendering
      Highlight
      Mermaid
      KaTeX
    Linting
      markdownlint
      Auto-fix
```

### 3.11 Timeline

```mermaid
timeline
    title Project history
    2024 : First release : Rushdown core
    2025 : syntect themes : Mermaid server-side
    2026 : Themed diagrams : Single theme kwarg
```

### 3.12 Quadrant chart

```mermaid
quadrantChart
    title Feature priority
    x-axis Low effort --> High effort
    y-axis Low value --> High value
    quadrant-1 Quick wins
    quadrant-2 Big bets
    quadrant-3 Fill-ins
    quadrant-4 Thankless
    "Themed Mermaid": [0.7, 0.9]
    "Server SVG": [0.4, 0.8]
```

---

## 4. Mathematics (KaTeX)

A display block via the `math` fence:

```math
\int_{0}^{\infty} e^{-x^2}\,dx = \frac{\sqrt{\pi}}{2}
```

The same formula via the `latex` fence:

```latex
\sum_{i=1}^{n} i = \frac{n(n+1)}{2}
```

And a centered display equation between `$$`:

$$
\nabla \times \vec{B} - \frac{1}{c}\frac{\partial \vec{E}}{\partial t}
= \frac{4\pi}{c}\vec{J}
\qquad
\nabla \cdot \vec{E} = 4\pi\rho
$$

Inline math is also supported, e.g. the quadratic formula
$x = \frac{-b \pm \sqrt{b^2 - 4ac}}{2a}$ appears right in the text.

---

## 5. Emoji shortcodes :heart: :tada: :fire:

Shortcodes are resolved to Unicode: `:wave:` → :wave:, `:rocket:` → :rocket:,
`:bulb:` → :bulb:, `:warning:` → :warning:, `:white_check_mark:` → :white_check_mark:.

On a line together: :star: :zap: :coffee: :lock: :unlock: :rocket: :sparkles:

---

## 6. Footnotes[^1] and references[^note]

Footnotes are always enabled. You can reference the same note twice[^note] to get
multiple back-links.

[^1]: The first footnote, defined at the bottom of the document.
[^note]: A named footnote used more than once in the body text.

---

## 7. Images (inline SVG, renders offline)

![Mordant badge](data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='140' height='44'><rect width='140' height='44' rx='10' fill='%2342138b'/><text x='12' y='28' fill='white' font-family='sans-serif' font-size='20'>mordant</text></svg>)

---

## 8. Thematic break & misc

Above is a thematic break. Here is a fenced `html` block (highlighted, not
executed, since `allows_unsafe` is off by default):

```html
<div class="callout">
  <strong>Note:</strong> raw HTML is escaped unless allows_unsafe is set.
</div>
```

And a final paragraph with a `code span`, a [second link](https://example.com),
and a closing emoji :wave:.

Happy documenting! :tada:
