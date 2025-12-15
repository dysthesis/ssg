# Code Samples Collection

This document contains code samples in various programming languages to test syntax highlighting performance.

## Rust

```rust
use std::collections::HashMap;
use color_eyre::Result;

/// Processes a list of items and returns a frequency map
fn count_frequencies<T: Eq + std::hash::Hash + Clone>(items: &[T]) -> HashMap<T, usize> {
    let mut freq_map = HashMap::new();
    for item in items {
        *freq_map.entry(item.clone()).or_insert(0) += 1;
    }
    freq_map
}

#[derive(Debug, Clone)]
struct Document {
    title: String,
    content: Vec<String>,
    metadata: HashMap<String, String>,
}

impl Document {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            content: Vec::new(),
            metadata: HashMap::new(),
        }
    }

    pub fn add_section(&mut self, section: impl Into<String>) {
        self.content.push(section.into());
    }
}

fn main() -> Result<()> {
    let mut doc = Document::new("Example");
    doc.add_section("Introduction");
    doc.add_section("Implementation");

    println!("Document: {:?}", doc);
    Ok(())
}
```

## Python

```python
from typing import Dict, List, Optional
import asyncio
from dataclasses import dataclass
from pathlib import Path

@dataclass
class Configuration:
    """Application configuration."""
    input_dir: Path
    output_dir: Path
    max_workers: int = 4
    verbose: bool = False

class MarkdownProcessor:
    def __init__(self, config: Configuration):
        self.config = config
        self.processed_count = 0

    async def process_file(self, path: Path) -> Optional[str]:
        """Process a single markdown file."""
        try:
            content = await asyncio.to_thread(path.read_text)
            result = self._transform(content)
            self.processed_count += 1
            return result
        except Exception as e:
            print(f"Error processing {path}: {e}")
            return None

    def _transform(self, content: str) -> str:
        # Placeholder transformation
        return f"<html>{content}</html>"

async def main():
    config = Configuration(
        input_dir=Path("input"),
        output_dir=Path("output"),
    )
    processor = MarkdownProcessor(config)

    files = list(config.input_dir.glob("*.md"))
    results = await asyncio.gather(*[processor.process_file(f) for f in files])

    print(f"Processed {processor.processed_count} files")

if __name__ == "__main__":
    asyncio.run(main())
```

## JavaScript

```javascript
/**
 * A simple virtual DOM implementation
 */
class VNode {
  constructor(tag, props, children) {
    this.tag = tag;
    this.props = props || {};
    this.children = children || [];
  }

  render() {
    const element = document.createElement(this.tag);

    // Apply properties
    Object.entries(this.props).forEach(([key, value]) => {
      if (key.startsWith('on')) {
        const event = key.substring(2).toLowerCase();
        element.addEventListener(event, value);
      } else {
        element.setAttribute(key, value);
      }
    });

    // Render children
    this.children.forEach(child => {
      if (typeof child === 'string') {
        element.appendChild(document.createTextNode(child));
      } else {
        element.appendChild(child.render());
      }
    });

    return element;
  }
}

// Usage example
const app = new VNode('div', { class: 'container' }, [
  new VNode('h1', {}, ['Hello World']),
  new VNode('p', {}, ['This is a virtual DOM example']),
  new VNode('button', {
    onClick: () => console.log('Clicked!')
  }, ['Click me'])
]);

document.body.appendChild(app.render());
```

## Go

```go
package main

import (
    "context"
    "fmt"
    "log"
    "sync"
    "time"
)

type Worker struct {
    id      int
    tasks   <-chan Task
    results chan<- Result
    wg      *sync.WaitGroup
}

type Task struct {
    ID   int
    Data string
}

type Result struct {
    TaskID int
    Output string
    Error  error
}

func NewWorker(id int, tasks <-chan Task, results chan<- Result, wg *sync.WaitGroup) *Worker {
    return &Worker{
        id:      id,
        tasks:   tasks,
        results: results,
        wg:      wg,
    }
}

func (w *Worker) Start(ctx context.Context) {
    defer w.wg.Done()

    for {
        select {
        case <-ctx.Done():
            return
        case task, ok := <-w.tasks:
            if !ok {
                return
            }
            w.process(task)
        }
    }
}

func (w *Worker) process(task Task) {
    // Simulate work
    time.Sleep(100 * time.Millisecond)

    result := Result{
        TaskID: task.ID,
        Output: fmt.Sprintf("Worker %d processed: %s", w.id, task.Data),
    }

    w.results <- result
}

func main() {
    ctx := context.Background()
    tasks := make(chan Task, 10)
    results := make(chan Result, 10)

    var wg sync.WaitGroup

    // Start workers
    for i := 0; i < 3; i++ {
        wg.Add(1)
        worker := NewWorker(i, tasks, results, &wg)
        go worker.Start(ctx)
    }

    // Send tasks
    go func() {
        for i := 0; i < 10; i++ {
            tasks <- Task{ID: i, Data: fmt.Sprintf("task-%d", i)}
        }
        close(tasks)
    }()

    // Collect results
    go func() {
        wg.Wait()
        close(results)
    }()

    for result := range results {
        log.Printf("Result: %+v\n", result)
    }
}
```

## C++

```cpp
#include <iostream>
#include <memory>
#include <vector>
#include <algorithm>
#include <string>

template<typename T>
class CircularBuffer {
private:
    std::vector<T> buffer;
    size_t head;
    size_t tail;
    size_t capacity;
    bool full;

public:
    explicit CircularBuffer(size_t size)
        : buffer(size), head(0), tail(0), capacity(size), full(false) {}

    void push(const T& item) {
        buffer[head] = item;

        if (full) {
            tail = (tail + 1) % capacity;
        }

        head = (head + 1) % capacity;
        full = head == tail;
    }

    bool pop(T& item) {
        if (empty()) {
            return false;
        }

        item = buffer[tail];
        full = false;
        tail = (tail + 1) % capacity;
        return true;
    }

    bool empty() const {
        return (!full && (head == tail));
    }

    size_t size() const {
        if (full) {
            return capacity;
        }

        if (head >= tail) {
            return head - tail;
        }

        return capacity + head - tail;
    }
};

int main() {
    CircularBuffer<int> buffer(5);

    for (int i = 0; i < 7; ++i) {
        buffer.push(i);
        std::cout << "Pushed: " << i << ", Size: " << buffer.size() << std::endl;
    }

    int value;
    while (buffer.pop(value)) {
        std::cout << "Popped: " << value << std::endl;
    }

    return 0;
}
```

## SQL

```sql
-- Complex query with multiple joins, CTEs, and window functions
WITH monthly_sales AS (
    SELECT
        DATE_TRUNC('month', order_date) AS month,
        customer_id,
        SUM(total_amount) AS total_sales,
        COUNT(DISTINCT order_id) AS order_count
    FROM orders
    WHERE order_date >= '2024-01-01'
        AND status = 'completed'
    GROUP BY DATE_TRUNC('month', order_date), customer_id
),
customer_rankings AS (
    SELECT
        month,
        customer_id,
        total_sales,
        order_count,
        ROW_NUMBER() OVER (PARTITION BY month ORDER BY total_sales DESC) AS sales_rank,
        LAG(total_sales) OVER (PARTITION BY customer_id ORDER BY month) AS prev_month_sales
    FROM monthly_sales
)
SELECT
    cr.month,
    c.customer_name,
    c.email,
    cr.total_sales,
    cr.order_count,
    cr.sales_rank,
    ROUND(((cr.total_sales - COALESCE(cr.prev_month_sales, 0))
        / NULLIF(cr.prev_month_sales, 0) * 100), 2) AS growth_percentage,
    CASE
        WHEN cr.sales_rank <= 10 THEN 'Top Tier'
        WHEN cr.sales_rank <= 50 THEN 'Mid Tier'
        ELSE 'Standard'
    END AS customer_tier
FROM customer_rankings cr
JOIN customers c ON cr.customer_id = c.id
WHERE cr.sales_rank <= 100
ORDER BY cr.month DESC, cr.sales_rank ASC;
```

## Shell Script

```bash
#!/usr/bin/env bash
set -euo pipefail

# Deployment script with error handling and logging

readonly SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
readonly LOG_FILE="/var/log/deploy.log"
readonly BACKUP_DIR="/var/backups"

log() {
    echo "[$(date +'%Y-%m-%d %H:%M:%S')] $*" | tee -a "$LOG_FILE"
}

error() {
    log "ERROR: $*" >&2
    exit 1
}

backup_database() {
    local db_name="$1"
    local backup_file="${BACKUP_DIR}/${db_name}_$(date +%Y%m%d_%H%M%S).sql"

    log "Creating database backup: $backup_file"

    if pg_dump "$db_name" > "$backup_file"; then
        log "Backup created successfully"
        gzip "$backup_file"
    else
        error "Database backup failed"
    fi
}

deploy_application() {
    local app_dir="$1"
    local service_name="$2"

    log "Deploying application in $app_dir"

    cd "$app_dir"

    # Pull latest code
    git pull origin main || error "Git pull failed"

    # Install dependencies
    if [ -f "package.json" ]; then
        npm install --production || error "npm install failed"
    elif [ -f "Cargo.toml" ]; then
        cargo build --release || error "cargo build failed"
    fi

    # Restart service
    sudo systemctl restart "$service_name" || error "Service restart failed"

    log "Deployment completed successfully"
}

main() {
    if [ $# -lt 2 ]; then
        error "Usage: $0 <app_dir> <service_name>"
    fi

    local app_dir="$1"
    local service_name="$2"

    backup_database "production_db"
    deploy_application "$app_dir" "$service_name"

    log "All operations completed"
}

main "$@"
```
