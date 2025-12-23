if (window.FilePond) {
    if (window.FilePondPluginImagePreview) {
        FilePond.registerPlugin(FilePondPluginImagePreview)
    }

    document.querySelectorAll("[data-filepond]").forEach((fp) => {
        window.FilePond.create(fp, { credits: false })
    })
}
