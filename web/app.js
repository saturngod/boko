// DOM elements
const dropzone = document.getElementById('dropzone');
const fileInput = document.getElementById('file-input');
const fileInfo = document.getElementById('file-info');
const fileName = document.getElementById('file-name');
const fileSize = document.getElementById('file-size');
const clearFileBtn = document.getElementById('clear-file');
const conversionOptions = document.getElementById('conversion-options');
const outputFormat = document.getElementById('output-format');
const convertBtn = document.getElementById('convert-btn');
const progress = document.getElementById('progress');
const progressFill = document.getElementById('progress-fill');
const progressText = document.getElementById('progress-text');
const result = document.getElementById('result');
const downloadLink = document.getElementById('download-link');
const error = document.getElementById('error');
const errorMessage = document.getElementById('error-message');
const stepLabel = document.getElementById('step-label');

let wasmReady = false;
let wasmPromise = null;
let converters = null;
let currentFile = null;
let downloadUrl = null;

// Load the conversion engine only after a file is selected. This keeps the
// initial page lightweight and avoids downloading ~2 MB for casual visitors.
async function initWasm() {
    if (wasmReady) return;
    if (wasmPromise) return wasmPromise;

    convertBtn.disabled = true;
    convertBtn.textContent = 'Preparing…';

    wasmPromise = import('./pkg/boko.js').then(async (boko) => {
        await boko.default();
        converters = {
            'epub_azw3': boko.epub_to_azw3,
            'epub_kfx': boko.epub_to_kfx,
            'epub_markdown': boko.epub_to_markdown,
            'azw3_epub': boko.azw3_to_epub,
            'azw3_markdown': boko.azw3_to_markdown,
            'kfx_epub': boko.kfx_to_epub,
            'kfx_markdown': boko.kfx_to_markdown,
            'mobi_epub': boko.mobi_to_epub,
            'mobi_azw3': boko.mobi_to_azw3,
            'mobi_markdown': boko.mobi_to_markdown,
        };
        wasmReady = true;
    }).catch((e) => {
        wasmPromise = null;
        showError('Failed to load the converter: ' + e.message);
        throw e;
    }).finally(() => {
        convertBtn.disabled = false;
        convertBtn.textContent = 'Convert';
    });

    return wasmPromise;
}

// File handling
function getFileExtension(filename) {
    return filename.split('.').pop().toLowerCase();
}

function formatFileSize(bytes) {
    if (bytes < 1024) return bytes + ' B';
    if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + ' KB';
    return (bytes / (1024 * 1024)).toFixed(1) + ' MB';
}

function getInputFormat(filename) {
    const ext = getFileExtension(filename);
    if (ext === 'epub') return 'epub';
    if (ext === 'azw3') return 'azw3';
    if (ext === 'kfx') return 'kfx';
    if (ext === 'mobi') return 'mobi';
    return null;
}

function updateOutputOptions(inputFormat) {
    outputFormat.innerHTML = '';

    if (inputFormat === 'epub') {
        outputFormat.innerHTML = `
            <option value="azw3">AZW3</option>
            <option value="kfx">KFX</option>
            <option value="markdown">Markdown</option>
        `;
    } else if (inputFormat === 'azw3') {
        outputFormat.innerHTML = `
            <option value="epub">EPUB</option>
            <option value="markdown">Markdown</option>
        `;
    } else if (inputFormat === 'kfx') {
        outputFormat.innerHTML = `
            <option value="epub">EPUB</option>
            <option value="markdown">Markdown</option>
        `;
    } else if (inputFormat === 'mobi') {
        outputFormat.innerHTML = `
            <option value="epub">EPUB</option>
            <option value="azw3">AZW3</option>
            <option value="markdown">Markdown</option>
        `;
    }
}

function handleFile(file) {
    const inputFormat = getInputFormat(file.name);

    if (!inputFormat) {
        showError('Unsupported file format. Please use EPUB, AZW3, MOBI, or KFX.');
        return;
    }

    currentFile = file;

    // Update UI
    fileName.textContent = file.name;
    fileSize.textContent = formatFileSize(file.size);
    fileInfo.classList.remove('hidden');
    dropzone.classList.add('hidden');

    updateOutputOptions(inputFormat);
    conversionOptions.classList.remove('hidden');
    stepLabel.textContent = 'Step 2 of 2';

    // Hide previous results/errors
    result.classList.add('hidden');
    error.classList.add('hidden');

    // Start loading in the background while the user chooses an output format.
    initWasm().catch(() => {});
}

function clearFile() {
    currentFile = null;
    fileInput.value = '';

    fileInfo.classList.add('hidden');
    conversionOptions.classList.add('hidden');
    result.classList.add('hidden');
    error.classList.add('hidden');
    dropzone.classList.remove('hidden');
    stepLabel.textContent = 'Step 1 of 2';

    if (downloadUrl) {
        URL.revokeObjectURL(downloadUrl);
        downloadUrl = null;
    }
}

function showError(message) {
    errorMessage.textContent = message;
    error.classList.remove('hidden');
    progress.classList.add('hidden');
    result.classList.add('hidden');
}

function showProgress(message) {
    progressText.textContent = message;
    progressFill.style.width = '50%';
    progress.classList.remove('hidden');
    result.classList.add('hidden');
    error.classList.add('hidden');
}

function showResult(blob, filename) {
    if (downloadUrl) URL.revokeObjectURL(downloadUrl);
    downloadUrl = URL.createObjectURL(blob);
    downloadLink.href = downloadUrl;
    downloadLink.download = filename;

    progress.classList.add('hidden');
    result.classList.remove('hidden');
}

const mimeTypes = {
    'epub': 'application/epub+zip',
    'azw3': 'application/x-mobi8-ebook',
    'kfx': 'application/x-kfx-ebook',
    'markdown': 'text/markdown',
};

const extensions = {
    'epub': '.epub',
    'azw3': '.azw3',
    'kfx': '.kfx',
    'markdown': '.md',
};

async function convert() {
    if (!wasmReady) {
        showProgress('Preparing converter…');
        try {
            await initWasm();
        } catch {
            return;
        }
    }

    if (!currentFile) {
        showError('No file selected.');
        return;
    }

    const inputFormat = getInputFormat(currentFile.name);
    const targetFormat = outputFormat.value;
    const converterKey = `${inputFormat}_${targetFormat}`;
    const converter = converters[converterKey];

    if (!converter) {
        showError('Unsupported conversion: ' + inputFormat + ' to ' + targetFormat);
        return;
    }

    showProgress('Reading file...');

    try {
        const arrayBuffer = await currentFile.arrayBuffer();
        const inputData = new Uint8Array(arrayBuffer);

        showProgress('Converting...');

        const outputData = converter(inputData);
        const baseName = currentFile.name.replace(/\.[^/.]+$/, '');
        const outputFilename = baseName + extensions[targetFormat];
        const mimeType = mimeTypes[targetFormat];

        const blob = new Blob([outputData], { type: mimeType });
        showResult(blob, outputFilename);

    } catch (e) {
        showError('Conversion failed: ' + e.message);
    }
}

// Event listeners
dropzone.addEventListener('click', () => fileInput.click());

dropzone.addEventListener('keydown', (e) => {
    if (e.key === 'Enter' || e.key === ' ') {
        e.preventDefault();
        fileInput.click();
    }
});

dropzone.addEventListener('dragover', (e) => {
    e.preventDefault();
    dropzone.classList.add('dragover');
});

dropzone.addEventListener('dragleave', () => {
    dropzone.classList.remove('dragover');
});

dropzone.addEventListener('drop', (e) => {
    e.preventDefault();
    dropzone.classList.remove('dragover');

    const files = e.dataTransfer.files;
    if (files.length > 0) {
        handleFile(files[0]);
    }
});

fileInput.addEventListener('change', (e) => {
    if (e.target.files.length > 0) {
        handleFile(e.target.files[0]);
    }
});

clearFileBtn.addEventListener('click', clearFile);
convertBtn.addEventListener('click', convert);
