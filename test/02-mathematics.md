# Mathematical Expressions

This document tests mathematical rendering capabilities using various formulas and equations.

## Inline Mathematics

The quadratic formula is given by $x = \frac{-b \pm \sqrt{b^2 - 4ac}}{2a}$, where $a$, $b$, and $c$ are coefficients.

Einstein's famous equation $E = mc^2$ relates energy and mass. The Pythagorean theorem states that $a^2 + b^2 = c^2$ for right triangles.

The sum of an arithmetic series is $S_n = \frac{n}{2}(a_1 + a_n)$ where $n$ is the number of terms.

## Display Mathematics

### Calculus

The derivative of a function:

$$\frac{d}{dx}f(x) = \lim_{h \to 0} \frac{f(x+h) - f(x)}{h}$$

The fundamental theorem of calculus:

$$\int_a^b f(x)\,dx = F(b) - F(a)$$

Taylor series expansion:

$$f(x) = f(a) + f'(a)(x-a) + \frac{f''(a)}{2!}(x-a)^2 + \frac{f'''(a)}{3!}(x-a)^3 + \cdots$$

### Linear Algebra

Matrix multiplication:

$$\begin{bmatrix} a & b \\ c & d \end{bmatrix} \begin{bmatrix} x \\ y \end{bmatrix} = \begin{bmatrix} ax + by \\ cx + dy \end{bmatrix}$$

Eigenvalue equation:

$$A\vec{v} = \lambda\vec{v}$$

Determinant of a 2×2 matrix:

$$\det(A) = \begin{vmatrix} a & b \\ c & d \end{vmatrix} = ad - bc$$

### Probability and Statistics

Probability density function of normal distribution:

$$f(x | \mu, \sigma^2) = \frac{1}{\sigma\sqrt{2\pi}} e^{-\frac{(x-\mu)^2}{2\sigma^2}}$$

Bayes' theorem:

$$P(A|B) = \frac{P(B|A)P(A)}{P(B)}$$

Expected value:

$$E[X] = \sum_{i=1}^{n} x_i P(x_i)$$

Variance:

$$\text{Var}(X) = E[(X - \mu)^2] = E[X^2] - (E[X])^2$$

### Complex Numbers

Euler's formula:

$$e^{ix} = \cos(x) + i\sin(x)$$

This leads to Euler's identity:

$$e^{i\pi} + 1 = 0$$

### Set Theory

Set operations:

$$A \cup B = \{x : x \in A \text{ or } x \in B\}$$

$$A \cap B = \{x : x \in A \text{ and } x \in B\}$$

$$A \setminus B = \{x : x \in A \text{ and } x \notin B\}$$

### Number Theory

Fermat's Little Theorem states that if $p$ is prime and $a$ is not divisible by $p$:

$$a^{p-1} \equiv 1 \pmod{p}$$

The prime number theorem:

$$\pi(x) \sim \frac{x}{\ln(x)}$$

### Summations and Products

Geometric series:

$$\sum_{k=0}^{n} ar^k = a\frac{1-r^{n+1}}{1-r}$$

Infinite geometric series (when $|r| < 1$):

$$\sum_{k=0}^{\infty} ar^k = \frac{a}{1-r}$$

Product notation:

$$n! = \prod_{k=1}^{n} k$$

Stirling's approximation:

$$n! \approx \sqrt{2\pi n}\left(\frac{n}{e}\right)^n$$

### Differential Equations

General solution to $y'' + \omega^2 y = 0$:

$$y(t) = A\cos(\omega t) + B\sin(\omega t)$$

Heat equation:

$$\frac{\partial u}{\partial t} = \alpha \frac{\partial^2 u}{\partial x^2}$$

Wave equation:

$$\frac{\partial^2 u}{\partial t^2} = c^2 \frac{\partial^2 u}{\partial x^2}$$

### Special Functions

Gamma function:

$$\Gamma(z) = \int_0^{\infty} t^{z-1}e^{-t}\,dt$$

Binomial coefficient:

$$\binom{n}{k} = \frac{n!}{k!(n-k)!}$$

Binomial theorem:

$$(x + y)^n = \sum_{k=0}^{n} \binom{n}{k} x^{n-k} y^k$$

## Systems of Equations

Linear system:

$$\begin{cases}
2x + 3y = 7 \\
4x - y = 5
\end{cases}$$

Solution using Cramer's rule:

$$x = \frac{\begin{vmatrix} 7 & 3 \\ 5 & -1 \end{vmatrix}}{\begin{vmatrix} 2 & 3 \\ 4 & -1 \end{vmatrix}}, \quad y = \frac{\begin{vmatrix} 2 & 7 \\ 4 & 5 \end{vmatrix}}{\begin{vmatrix} 2 & 3 \\ 4 & -1 \end{vmatrix}}$$

## Limits

Definition of a limit:

$$\lim_{x \to a} f(x) = L \iff \forall \epsilon > 0, \exists \delta > 0 : 0 < |x - a| < \delta \implies |f(x) - L| < \epsilon$$

L'Hôpital's rule for indeterminate forms:

$$\lim_{x \to a} \frac{f(x)}{g(x)} = \lim_{x \to a} \frac{f'(x)}{g'(x)}$$

## Vector Calculus

Gradient:

$$\nabla f = \frac{\partial f}{\partial x}\mathbf{i} + \frac{\partial f}{\partial y}\mathbf{j} + \frac{\partial f}{\partial z}\mathbf{k}$$

Divergence:

$$\nabla \cdot \mathbf{F} = \frac{\partial F_x}{\partial x} + \frac{\partial F_y}{\partial y} + \frac{\partial F_z}{\partial z}$$

Curl:

$$\nabla \times \mathbf{F} = \begin{vmatrix} \mathbf{i} & \mathbf{j} & \mathbf{k} \\ \frac{\partial}{\partial x} & \frac{\partial}{\partial y} & \frac{\partial}{\partial z} \\ F_x & F_y & F_z \end{vmatrix}$$
