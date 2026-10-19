"""Materialize translations beside the test executable using declared inputs."""

def _locales_impl(ctx):
    output = ctx.actions.declare_directory("locales")
    manifest = ctx.actions.declare_file(ctx.label.name + ".json")
    ctx.actions.write(manifest, json.encode([file.path for file in ctx.files.srcs]))
    ctx.actions.run(
        executable = "/usr/bin/python3",
        arguments = [ctx.file._driver.path, manifest.path, output.path],
        inputs = ctx.files.srcs + [manifest, ctx.file._driver],
        outputs = [output],
        mnemonic = "PeiosutilsLocales",
    )
    return [DefaultInfo(files = depset([output]))]

locale_tree = rule(
    implementation = _locales_impl,
    attrs = {
        "srcs": attr.label_list(allow_files = [".ftl"]),
        "_driver": attr.label(default = "//tools:stage-locales.py", allow_single_file = True),
    },
    doc = "Stage all runtime translations without relying on a Cargo output layout.",
)
