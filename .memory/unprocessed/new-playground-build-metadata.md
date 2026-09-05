# New playgrounds use dependency-free and relocatable metadata

The Kotlin playground uses compiler-valid sources plus `settings.gradle.kts`, without adding a
Gradle or Maven Kotlin plugin dependency. CUDA, Objective-C, and Objective-C++ use relative
`compile_commands.json` working directories so their compilation metadata remains valid after the
E2E harness copies a project into a randomized sandbox.
