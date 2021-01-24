const button = document.getElementById('menu')
const menu = document.getElementById('content')

button.addEventListener('click', () => {
  menu.classList.add('show')
})

menu.addEventListener('click', () => {
  menu.classList.remove('show')
})
