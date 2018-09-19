<script src="//unpkg.com/swagger-ui-dist@3/swagger-ui-bundle.js"></script>

<div id="api"></div>

<script>
const ui = SwaggerUIBundle({
    url: "/Plume/api.yaml",
    dom_id: '#api',
    presets: [
        SwaggerUIBundle.presets.apis,
        SwaggerUIBundle.SwaggerUIStandalonePreset
    ],
    layout: "StandaloneLayout"
})
</script>
