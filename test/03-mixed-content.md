# Mixed Content Document

This document contains various Markdown elements including lists, tables, blockquotes, and more.

## Text Formatting

This is **bold text**, this is *italic text*, and this is ***bold and italic***.

You can also use ~~strikethrough~~ text and `inline code`.

Here's a [link to example.com](https://example.com) and an autolink: <https://www.github.com>

## Lists

### Unordered Lists

- First item
- Second item
  - Nested item 2.1
  - Nested item 2.2
    - Deeply nested item
    - Another deeply nested item
- Third item
- Fourth item

### Ordered Lists

1. First step
2. Second step
3. Third step
   1. Sub-step 3.1
   2. Sub-step 3.2
4. Fourth step

### Task Lists

- [x] Completed task
- [x] Another completed task
- [ ] Incomplete task
- [ ] Another incomplete task
  - [x] Completed sub-task
  - [ ] Incomplete sub-task

## Tables

### Simple Table

| Name          | Age | Occupation       |
|---------------|-----|------------------|
| Alice Johnson | 28  | Software Engineer|
| Bob Smith     | 34  | Data Scientist   |
| Carol White   | 29  | Product Manager  |
| David Brown   | 42  | Tech Lead        |

### Aligned Table

| Left Aligned | Center Aligned | Right Aligned |
|:-------------|:--------------:|--------------:|
| Apple        | $1.50          | 100           |
| Banana       | $0.75          | 250           |
| Orange       | $2.00          | 150           |
| Grape        | $3.50          | 80            |

### Complex Table

| Feature       | Free Tier | Pro Tier  | Enterprise    |
|---------------|-----------|-----------|---------------|
| Users         | Up to 5   | Up to 50  | Unlimited     |
| Storage       | 10 GB     | 100 GB    | 1 TB+         |
| API Calls     | 1,000/day | 10,000/day| Unlimited     |
| Support       | Community | Email     | 24/7 Phone    |
| Price/month   | $0        | $49       | Contact Sales |

## Blockquotes

> This is a simple blockquote.
> It can span multiple lines.

> This is a blockquote with multiple paragraphs.
>
> Here's the second paragraph in the blockquote.
>
> > This is a nested blockquote.
> > It's inside another blockquote.

> **Note:** You can use Markdown formatting inside blockquotes.
>
> - List item 1
> - List item 2
>
> ```python
> def example():
>     return "code in blockquotes"
> ```

## Horizontal Rules

---

***

___

## Definition Lists

Term 1
: Definition 1a
: Definition 1b

Term 2
: Definition 2a
: Definition 2b

## Footnotes

Here's a sentence with a footnote.[^1]

Here's another one.[^2]

And a longer footnote.[^longnote]

[^1]: This is the first footnote.

[^2]: This is the second footnote.

[^longnote]: Here's one with multiple paragraphs.

    Indent paragraphs to include them in the footnote.

    Add as many paragraphs as you like.

## Abbreviations

The HTML specification is maintained by the W3C.

*[HTML]: Hyper Text Markup Language
*[W3C]: World Wide Web Consortium

## Code Blocks with Highlighting

Here's a Python example:

```python
def fibonacci(n):
    """Generate Fibonacci sequence up to n terms."""
    a, b = 0, 1
    result = []
    for _ in range(n):
        result.append(a)
        a, b = b, a + b
    return result

print(fibonacci(10))
```

And a TypeScript example:

```typescript
interface User {
  id: number;
  name: string;
  email: string;
  roles: string[];
}

class UserManager {
  private users: Map<number, User>;

  constructor() {
    this.users = new Map();
  }

  addUser(user: User): void {
    this.users.set(user.id, user);
  }

  getUser(id: number): User | undefined {
    return this.users.get(id);
  }

  hasRole(userId: number, role: string): boolean {
    const user = this.getUser(userId);
    return user?.roles.includes(role) ?? false;
  }
}
```

## Mathematics

Inline math: The area of a circle is $A = \pi r^2$.

Display math:

$$
\int_0^1 x^2 dx = \left[\frac{x^3}{3}\right]_0^1 = \frac{1}{3}
$$

## Emojis and Special Characters

Common emojis: 😀 🎉 🚀 💻 📊 ✅ ❌

Special characters: © ® ™ § ¶ † ‡ • ‰

Mathematical symbols: ∀ ∃ ∈ ∉ ⊂ ⊃ ∩ ∪ ∞ ≠ ≈ ≤ ≥

Arrows: ← → ↑ ↓ ↔ ↕ ⇐ ⇒ ⇔

## Nested Structures

1. First ordered item
   - Unordered sub-item
   - Another unordered sub-item
     1. Nested ordered item
     2. Another nested ordered item
        - Deep unordered item
        - Another deep unordered item

2. Second ordered item
   > A blockquote in a list
   >
   > With multiple paragraphs

   ```rust
   // Code block in a list
   fn main() {
       println!("Hello from a list!");
   }
   ```

3. Third ordered item
   | Column 1 | Column 2 |
   |----------|----------|
   | Data 1   | Data 2   |

## Long Paragraphs

Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat.

Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt mollit anim id est laborum.

Sed ut perspiciatis unde omnis iste natus error sit voluptatem accusantium doloremque laudantium, totam rem aperiam, eaque ipsa quae ab illo inventore veritatis et quasi architecto beatae vitae dicta sunt explicabo.

Nemo enim ipsam voluptatem quia voluptas sit aspernatur aut odit aut fugit, sed quia consequuntur magni dolores eos qui ratione voluptatem sequi nesciunt. Neque porro quisquam est, qui dolorem ipsum quia dolor sit amet, consectetur, adipisci velit.

## Multiple Code Blocks

Ruby:

```ruby
class BinarySearchTree
  attr_accessor :value, :left, :right

  def initialize(value)
    @value = value
    @left = nil
    @right = nil
  end

  def insert(new_value)
    if new_value <= value
      left.nil? ? self.left = BinarySearchTree.new(new_value) : left.insert(new_value)
    else
      right.nil? ? self.right = BinarySearchTree.new(new_value) : right.insert(new_value)
    end
  end

  def search(target)
    return true if value == target
    return left.search(target) if target < value && !left.nil?
    return right.search(target) if target > value && !right.nil?
    false
  end
end
```

Java:

```java
public class QuickSort {
    public static void quickSort(int[] arr, int low, int high) {
        if (low < high) {
            int pivotIndex = partition(arr, low, high);
            quickSort(arr, low, pivotIndex - 1);
            quickSort(arr, pivotIndex + 1, high);
        }
    }

    private static int partition(int[] arr, int low, int high) {
        int pivot = arr[high];
        int i = low - 1;

        for (int j = low; j < high; j++) {
            if (arr[j] <= pivot) {
                i++;
                swap(arr, i, j);
            }
        }

        swap(arr, i + 1, high);
        return i + 1;
    }

    private static void swap(int[] arr, int i, int j) {
        int temp = arr[i];
        arr[i] = arr[j];
        arr[j] = temp;
    }

    public static void main(String[] args) {
        int[] numbers = {64, 34, 25, 12, 22, 11, 90};
        quickSort(numbers, 0, numbers.length - 1);

        for (int num : numbers) {
            System.out.print(num + " ");
        }
    }
}
```
