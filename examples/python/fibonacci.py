def fibonacci(n):
    if n <= 1:
        return n
    return fibonacci(n-1) + fibonacci(n-2)

n = 10
print(f"Fibonacci sequence up to {n}:")
for i in range(n):
    print(f"F({i}) = {fibonacci(i)}")