FilePond.registerPlugin(FilePondPluginImagePreview)

window.addEventListener("DOMContentLoaded", function () {
    FilePond.create(document.querySelector("#simple-filepond-demo"), {
        allowImagePreview: false,
        credits: false,
    })
    FilePond.create(document.querySelector("#multiple-filepond-demo"), {
        credits: false,
        allowImageCrop: false,
        allowMultiple: true,
    })

    FilePond.create(document.querySelector("#image-preview-filepond-demo"), {
        allowImagePreview: true,
        credits: false,
    })

    FilePond.create(document.querySelector("#disabled-filepond-demo"), {
        allowImagePreview: false,
        credits: false,
        disabled: true,
    })

    FilePond.create(document.querySelector("#circle-filepond-demo"), {
        stylePanelAspectRatio: "1:1",
        stylePanelLayout: "compact circle",
        credits: false,
    })
})
