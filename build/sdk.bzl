"""Import the installed C and authentication SDKs through explicit declared inputs."""

_LIBRARIES = {"peios": "libpeios.so.0", "auth": "libauth.so.1"}

def _sdk_repository_impl(ctx):
    sdk = ctx.os.environ.get("PEIOS_SDK")
    if not sdk:
        ctx.file("missing.bzl", """
def _missing_impl(_ctx):
    fail("Set PEIOS_SDK to the installed SDK root; see docs/development/building.md.")
missing_sdk = rule(implementation = _missing_impl)
""")
        ctx.file("BUILD.bazel", """
load(":missing.bzl", "missing_sdk")
package(default_visibility = ["//visibility:public"])
""" + "\n".join(['missing_sdk(name = "%s")' % name for name in ["files", "root", "library", "peios", "auth", "auth-library"]]))
        return
    if not sdk.startswith("/"):
        fail("PEIOS_SDK must be an absolute directory.")
    for name in ["include/peios.h", "include/pkm"] + ["lib/" + name for name in _LIBRARIES.values()]:
        if not ctx.path(sdk + "/" + name).exists:
            fail("Missing Peios SDK input: " + sdk + "/" + name)
    ctx.symlink(sdk + "/include", "include")
    ctx.file("root", "")
    build = """
load("@rules_cc//cc:defs.bzl", "cc_import", "cc_library")
package(default_visibility = ["//visibility:public"])
exports_files(["root"])
filegroup(name = "library", srcs = ["lib/libpeios.so.0"])
filegroup(name = "auth-library", srcs = ["lib/libauth.so.1"])
filegroup(name = "files", srcs = glob(["include/**", "lib/**"]))
"""
    for name, soname in _LIBRARIES.items():
        ctx.symlink(sdk + "/lib/" + soname, "lib/" + soname)
        ctx.symlink("lib/" + soname, "lib/lib" + name + ".so")
        build += """
cc_import(
    name = "%s-link",
    shared_library = select({"%s": None, "%s": None, "//conditions:default": "lib/%s"}),
    interface_library = "lib/lib%s.so",
    system_provided = select({"%s": True, "%s": True, "//conditions:default": False}),
)
cc_library(
    name = "%s",
    hdrs = glob(["include/**"]),
    strip_include_prefix = "include",
    deps = [":%s-link"],
)
""" % (name, ctx.attr.native_config, ctx.attr.package_config, soname, name, ctx.attr.native_config, ctx.attr.package_config, name, name)
    ctx.file("BUILD.bazel", build)

sdk_repository = repository_rule(
    doc = "Import explicit headers, libpeios and libauth without host library discovery.",
    implementation = _sdk_repository_impl,
    environ = ["PEIOS_SDK"],
    attrs = {
        "native_config": attr.label(default = Label("//build:native")),
        "package_config": attr.label(default = Label("//build:packaged")),
    },
    local = True,
    configure = True,
)
