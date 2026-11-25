// Copy buttons for install commands. No tracking, no third-party requests.
document.querySelectorAll("[data-copy]").forEach((button) => {
  button.addEventListener("click", async () => {
    const text = document.getElementById(button.dataset.copy).textContent.trim();
    try {
      await navigator.clipboard.writeText(text);
      button.textContent = "Copied";
      button.dataset.done = "";
    } catch {
      button.textContent = "Select and copy";
    }
    setTimeout(() => { button.textContent = "Copy"; delete button.dataset.done; }, 2200);
  });
});
