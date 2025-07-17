// Calculates the factorial of a number
function factorial(n) {
    if (n === 0 || n === 1) {
        return 1;
    }
    return n * factorial(n - 1);
}

const button = document.querySelector('.button');

button.addEventListener('click', function () {
    const input = document.getElementById('number');
    const output = document.getElementById('result');

    let value = parseInt(input.value, 10);
    if (!isNaN(value)) {
        output.textContent = 'Factorial: ' + factorial(value);
    } else {
        output.textContent = 'Please enter a valid number.';
    }
});
