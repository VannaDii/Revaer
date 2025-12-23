import * as z from "https://cdn.jsdelivr.net/npm/zod/+esm"

// Form Validator
const createFormValidator = (schema, initialData = {}) => {
    const data = window.Alpine.reactive(initialData)
    const errors = window.Alpine.reactive({})

    const updateData = (name, value) => {
        data[name] = value
        validateField(name)
        console.info(name, value)
    }

    const setError = (name, error) => {
        if (error === null) {
            delete errors[name]
        } else {
            errors[name] = error
        }
    }

    const hasError = (name) => {
        return errors[name] !== undefined
    }

    const getError = (name) => {
        return errors[name]
    }

    const getData = (name) => {
        return data[name]
    }

    function validateField(name) {
        if (!name || !schema.shape.hasOwnProperty(name)) return

        const partialSchema = schema.pick({ [name]: true })
        const result = partialSchema.safeParse({ [name]: data[name] })

        if (result.success) {
            setError(name, null)
        } else {
            setError(name, result.error.issues[0].message)
        }
    }

    function handleSubmit(callback) {
        const res = schema.safeParse(data)
        if (!res.success) {
            res.error.issues.forEach((issue) => setError(issue.path[0], issue.message))
        } else {
            callback?.(res.data)
        }
    }

    return {
        errors,
        getData,
        updateData,
        hasError,
        getError,
        onSubmit: handleSubmit,
    }
}

// Usages:

window.useTextFormValidator = () => {
    const schema = z.object({
        optionalText: z.string().optional(),
        minimumText: z.string().min(3, "Text must be at least 3 characters"),
        maximumText: z.string().max(5, "Text must be at max 5 characters"),
    })

    const initialData = {
        optionalText: "",
        minimumText: "12",
        maximumText: "123456",
    }
    return createFormValidator(schema, initialData)
}

window.useNumberValidator = () => {
    const schema = z.object({
        optionalNumber: z.preprocess(
            (val) => (val === "" ? undefined : val),
            z.number().optional()
        ),
        minimumNumber: z.number().min(18, "You must be at least 18"),
        maximumNumber: z.number().max(99, "You must be at maximum 99"),
    })

    const initialData = {
        optionalNumber: "",
        minimumNumber: 17,
        maximumNumber: 100,
    }
    return createFormValidator(schema, initialData)
}

window.useEmailFormValidator = () => {
    const schema = z.object({
        requiredEmail: z.email("Invalid email address"),
        optionalEmail: z.preprocess(
            (val) => (val === "" ? undefined : val),
            z.email("Invalid email address").optional()
        ),
    })

    const initialData = {
        optionalEmail: "",
        requiredEmail: "",
    }
    return createFormValidator(schema, initialData)
}

window.useRadioFormValidator = () => {
    const schema = z.object({
        requiredRadio: z.enum(["male", "female", "other"], { message: "Select gender" }),
    })

    const initialData = {
        requiredRadio: "",
    }
    return createFormValidator(schema, initialData)
}

window.useCheckboxFormValidator = () => {
    const schema = z.object({
        requiredCheckbox: z.boolean().refine((val) => val, {
            message: "Accept terms to continue",
        }),
    })

    const initialData = {
        requiredCheckbox: false,
    }
    return createFormValidator(schema, initialData)
}

window.useToggleFormValidator = () => {
    const schema = z.object({
        requiredToggle: z.boolean().refine((val) => val, {
            message: "Accept terms to continue",
        }),
    })

    const initialData = {
        requiredToggle: false,
    }
    return createFormValidator(schema, initialData)
}

window.useSelectFormValidator = () => {
    const schema = z.object({
        requiredSelect: z.string().min(1, "Select a country"),
    })

    const initialData = {
        requiredSelect: "",
    }
    return createFormValidator(schema, initialData)
}

window.useRangeFormValidator = () => {
    const schema = z.object({
        betweenNumber: z
            .number()
            .min(20, "You must be at least 20")
            .max(80, "You must be at maximum 80"),
    })

    const initialData = {
        betweenNumber: 15,
    }
    return createFormValidator(schema, initialData)
}

window.useRatingsFormValidator = () => {
    const schema = z.object({
        requiredRating: z.preprocess(
            (e) => (e === undefined ? undefined : parseInt(e)),
            z.number("Please select a rating").min(1, "Please select a rating")
        ),
    })

    const initialData = {
        requiredRating: 0,
    }
    return createFormValidator(schema, initialData)
}

window.useFormFormValidator = () => {
    const schema = z.object({
        firstName: z
            .string("Please enter your first name")
            .min(2, "First name must be at least 2 characters long"),
        lastName: z
            .string("Please enter your last name")
            .min(2, "Last name must be at least 2 characters long"),
        username: z
            .string("Please enter your username")
            .min(2, "Username must be at least 2 characters long"),
        email: z.email("Invalid email address"),
        phoneNumber: z
            .string("Phone number is invalid")
            .regex(/^[0-9]{10}$/, "Phone number must be 10 digits long"),
        dob: z.iso.date("Invalid date of birth"),
    })

    const initialData = {
        firstName: "",
        lastName: "",
        username: "",
        email: "",
        phoneNumber: "",
        dob: new Date().toLocaleString(),
    }
    return createFormValidator(schema, initialData)
}
