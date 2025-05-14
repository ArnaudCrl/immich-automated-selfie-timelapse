document.addEventListener('DOMContentLoaded', function () {
    checkConnectionStatus();

    // Toggle framerate field visibility based on checkbox
    document.getElementById('do_not_compile_video').addEventListener('change', function () {
        document.getElementById('framerateGroup').style.display = this.checked ? 'none' : 'block';
    });
});

function checkConnectionStatus() {
    fetch('/check-connection')
        .then(response => response.json())
        .then(data => {
            const statusDiv = document.getElementById('connectionStatus');
            const submitButton = document.getElementById('submitButton');

            if (data.valid) {
                statusDiv.className = 'connection-status connected';
                statusDiv.textContent = 'Connected to Immich server';
                submitButton.disabled = false;
            } else {
                statusDiv.className = 'connection-status disconnected';
                statusDiv.textContent = 'Error: ' + data.message;
                submitButton.disabled = true;
            }
        })
        .catch(error => {
            console.error('Error checking connection:', error);
            const statusDiv = document.getElementById('connectionStatus');
            statusDiv.className = 'connection-status disconnected';
            statusDiv.textContent = 'Error checking Immich server connection';
            document.getElementById('submitButton').disabled = true;
        });
}

// Cancel button
document.getElementById('cancelButton').addEventListener('click', function () {
    fetch('/cancel', {
        method: 'POST'
    })
        .then(response => response.json())
        .then(data => {
            if (data.success) {
                document.getElementById('progressText').textContent = 'Processing cancelled.';
            }
        })
        .catch(error => console.error('Error:', error));
});

// Progress monitoring
let isProcessing = false;
function fetchProgress() {
    fetch('/progress')
        .then(response => response.json())
        .then(data => {
            const progressBar = document.getElementById("progressBar");
            const progressText = document.getElementById("progressText");
            const cancelButton = document.getElementById("cancelButton");
            const submitButton = document.getElementById("submitButton");
            const videoResult = document.getElementById("videoResult");
            const videoLink = document.getElementById("videoLink");

            // Show/hide cancel button based on status
            if (data.status === "running") {
                isProcessing = true;
                cancelButton.style.display = "block";
                submitButton.disabled = true;
            } else {
                isProcessing = false;
                cancelButton.style.display = "none";
                submitButton.disabled = false;
            }

            // Handle status messages
            if (data.status === "cancelled") {
                progressText.textContent = "Processing cancelled.";
                progressBar.value = progressBar.max;
            } else if (data.status === "done") {
                progressText.textContent = "Processing complete!";
                progressBar.value = progressBar.max;
            } else if (data.status === "compiling_video") {
                progressText.textContent = "Images generated, creating video";
            } else if (data.status === "video_done") {
                progressText.textContent = "Video compilation complete!";
                progressBar.value = progressBar.max;
            } else if (data.status.startsWith("error:")) {
                progressText.textContent = data.status;
                progressBar.value = progressBar.max;
            } else if (data.total > 0) {
                progressBar.max = data.total;
                progressBar.value = data.completed;
                const percent = Math.floor((data.completed / data.total) * 100);
                progressText.textContent = percent + "% (" + data.completed + "/" + data.total + ")";
            }
        })
        .catch(err => console.error('Error fetching progress:', err));
}

// Poll every 1 second
setInterval(fetchProgress, 200);

// Form submission
document.getElementById('timelapseForm').addEventListener('submit', function () {
    document.getElementById("videoResult").style.display = "none";
});
