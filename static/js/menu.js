document.addEventListener("DOMContentLoaded", () => {
	let content = document.getElementById("content");
	document.getElementById("menu").querySelector("a").addEventListener("click", () => {
		content.classList.add("opened");
	});
	content.addEventListener("click", (e) => {
		if (e.target === content)
			content.classList.remove("opened");
	});
});