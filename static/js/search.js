window.onload = function(evt) {
  var form = document.getElementById('form');
  form.addEventListener('submit', function () {
    for (var input of form.getElementsByTagName('input')) {
      if (input.name === '') {
        input.name = input.id
      }
      if (input.name && !input.value) {
        input.name = '';
      }
    }
  });
}
