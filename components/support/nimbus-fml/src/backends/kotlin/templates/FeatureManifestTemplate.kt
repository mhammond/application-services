// This file was autogenerated by some hot garbage in the `nimbus-fml` crate.
// Trust me, you don't want to mess with it!

{%- match self.fm.about.nimbus_package_name() %}
{%- when Some with (package_name) %}
package {{ package_name }}

{% else -%}
{% endmatch %}

{%- for imported_class in self.imports() %}
import {{ imported_class }}
{%- endfor %}

{%- let nimbus_object = self.fm.about.nimbus_object_name() %}

/**
 * An object for safely accessing feature configuration from Nimbus.
 *
 * This is generated.
 *
 * Before use to configure the application or any of its features, this class needs
 * to be wired up to the SDK API. This is an object created by the application which connects to
 * the Nimbus SDK and thence to the server.
 *
 * ```
 * val nimbus: Nimbus = connectToNimbusSDK()
 * {{ nimbus_object }}.initialize(getSdk = { nimbus })
 * ```
 *
 * Once initialized, this can be used to access typesafe configuration object via the `features` member.
 *
 * This class should not be edited manually, but changed by editing the `nimbus.fml.yaml` file, and
 * re-running the `nimbus-fml` tool, which is likely already being used by the build script.
 */
object {{ nimbus_object }} {
    class Features {
        {%- for f in self.iter_feature_defs() %}
        {%- let raw_name = f.name() %}
        {%- let class_name = raw_name|class_name %}
        {{ f.doc()|comment("        ") }}
        val {{raw_name|var_name}}: FeatureHolder<{{class_name}}> by lazy {
            FeatureHolder({{ nimbus_object }}.getSdk, {{ raw_name|quoted }}) { variables ->
                {{ class_name }}(variables)
            }
        }
        {%- endfor %}
    }

    /**
     * This method should be called as early in the startup sequence of the app as possible.
     * This is to connect the Nimbus SDK (and thus server) with the `{{ nimbus_object }}`
     * class.
     *
     * The lambda MUST be threadsafe in its own right.
     */
    public fun initialize(getSdk: () -> FeaturesInterface?) {
        this.getSdk = getSdk
    }

    private var getSdk: () -> FeaturesInterface? = {
        this.api
    }

    /**
     * This is the connection between the Nimbus SDK (and thus the Nimbus server) and the generated code.
     *
     * This is no longer the recommended way of doing this, and will be removed in future releases.
     *
     * The recommended method is to use the `initialize(getSdk)` method, much earlier in the application
     * startup process.
     */
    public var api: FeaturesInterface? = null

    public fun invalidateCachedValues() {
        {% for f in self.iter_feature_defs() -%}
        features.{{- f.name()|var_name -}}.withCachedValue(null)
        {% endfor %}
    }

    /**
     * Accessor object for generated configuration classes extracted from Nimbus, with built-in
     * default values.
     */
    val features = Features()
}

{%- for code in self.initialization_code() %}
{{ code }}
{%- endfor %}

// Public interface members begin here.
{% for code in self.declaration_code() %}
{{- code }}
{%- endfor %}

{% import "macros.kt" as kt %}