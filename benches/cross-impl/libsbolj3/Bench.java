// Round-trip benchmark wrapper for libSBOLj3.
//
// Reads an SBOL 3 RDF document, runs the configured number of warmup
// and timed (parse + serialize) iterations, and writes per-iteration
// nanosecond timings to a JSON file. The output-file convention keeps
// the timings clean even when libSBOLj3's slf4j/hibernate loggers
// decide to print INFO lines on startup.
//
// Invoked from inside the Docker container built by the sibling
// Dockerfile. The optional trailing `validate` flag (`1`/`0`) turns on
// a timed `SBOLValidator.getValidator().validate(doc)` phase whose
// per-iteration timings are emitted under `validate_ns`.

import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import java.io.FileWriter;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.Properties;
import java.util.List;

import org.sbolstandard.core3.entity.SBOLDocument;
import org.sbolstandard.core3.io.SBOLFormat;
import org.sbolstandard.core3.io.SBOLIO;
import org.sbolstandard.core3.validation.SBOLValidator;
import org.sbolstandard.core3.util.Configuration;

public class Bench {

    public static void main(String[] args) throws Exception {
        if (args.length < 6) {
            System.err.println(
                "usage: Bench <input> <parse_fmt> <serialize_fmt> <warmup> <iters> <output_json>");
            System.exit(2);
        }

        Path inputPath = Paths.get(args[0]);
        SBOLFormat parseFormat = parseFormat(args[1]);
        SBOLFormat serializeFormat = parseFormat(args[2]);
        int warmup = Integer.parseInt(args[3]);
        int iters = Integer.parseInt(args[4]);
        Path outputJson = Paths.get(args[5]);
        boolean validate = args.length > 6 && args[6].equals("1");

        // Separate document I/O from explicit validation. These three flags
        // otherwise make parse/write include hidden validation passes.
        Configuration configuration = Configuration.getInstance();
        configuration.setValidateAfterSettingProperties(false);
        configuration.setValidateAfterReadingSBOLDocuments(false);
        configuration.setValidateBeforeSaving(false);
        configuration.setValidateRecommendedRules(true);
        configuration.setCompleteDocument(true);

        byte[] rdfBytes = Files.readAllBytes(inputPath);
        List<String> expectedMessages = validate ? SBOLValidator.getValidator().validate(
            SBOLIO.read(new ByteArrayInputStream(rdfBytes), parseFormat)) : List.of();

        long[] parseNs = new long[iters];
        long[] serializeNs = new long[iters];
        long[] validateNs = validate ? new long[iters] : new long[0];
        long lastSerializedBytes = 0L;

        for (int i = 0; i < warmup; i++) {
            SBOLDocument doc = SBOLIO.read(new ByteArrayInputStream(rdfBytes), parseFormat);
            if (validate) {
                checkMessages(expectedMessages, SBOLValidator.getValidator().validate(doc));
            }
            ByteArrayOutputStream out = new ByteArrayOutputStream();
            SBOLIO.write(doc, out, serializeFormat);
        }

        for (int i = 0; i < iters; i++) {
            long t0 = System.nanoTime();
            SBOLDocument doc = SBOLIO.read(new ByteArrayInputStream(rdfBytes), parseFormat);
            long t1 = System.nanoTime();
            parseNs[i] = t1 - t0;
            // Validate the freshly parsed document, before serialization.
            if (validate) {
                long v0 = System.nanoTime();
                List<String> messages = SBOLValidator.getValidator().validate(doc);
                long v1 = System.nanoTime();
                validateNs[i] = v1 - v0;
                checkMessages(expectedMessages, messages);
            }
            long s0 = System.nanoTime();
            ByteArrayOutputStream out = new ByteArrayOutputStream();
            SBOLIO.write(doc, out, serializeFormat);
            long s1 = System.nanoTime();
            serializeNs[i] = s1 - s0;
            lastSerializedBytes = out.size();
        }

        String impl = "libsbolj3";
        String version = libsbolj3Version();
        StringBuilder json = new StringBuilder(1024 + iters * 16);
        json.append("{");
        appendStringField(json, "impl", impl).append(",");
        appendStringField(json, "version", version).append(",");
        appendStringField(json, "fixture", inputPath.toString()).append(",");
        appendStringField(json, "parse_format", args[1]).append(",");
        appendStringField(json, "serialize_format", args[2]).append(",");
        appendLongField(json, "warmup_iters", warmup).append(",");
        appendLongField(json, "measured_iters", iters).append(",");
        appendLongField(json, "serialized_bytes", lastSerializedBytes).append(",");
        appendLongArrayField(json, "parse_ns", parseNs).append(",");
        appendLongArrayField(json, "serialize_ns", serializeNs);
        if (validate) {
            json.append(",");
            appendLongArrayField(json, "validate_ns", validateNs);
            json.append(",\"validation\":{\"complete\":true,\"recommended\":true,\"automatic_validation\":false,\"ontology\":\"bundled EDAM/SO/SBO sets\",\"messages\":[");
            for (int i = 0; i < expectedMessages.size(); i++) {
                if (i > 0) json.append(',');
                json.append('"').append(escape(expectedMessages.get(i))).append('"');
            }
            json.append("]}");
        }
        json.append("}");

        try (FileWriter writer = new FileWriter(outputJson.toFile(), StandardCharsets.UTF_8)) {
            writer.write(json.toString());
        }
    }

    private static void checkMessages(List<String> expected, List<String> actual) {
        if (!new java.util.HashSet<>(expected).equals(new java.util.HashSet<>(actual))
                || expected.size() != actual.size()) {
            throw new IllegalStateException("validation diagnostics changed across iterations");
        }
    }

    private static SBOLFormat parseFormat(String name) {
        switch (name.toLowerCase()) {
            case "turtle":
                return SBOLFormat.TURTLE;
            case "rdfxml":
                return SBOLFormat.RDFXML;
            case "jsonld":
                return SBOLFormat.JSONLD;
            case "ntriples":
                return SBOLFormat.NTRIPLES;
            default:
                System.err.println("unknown format: " + name);
                System.exit(2);
                return null;
        }
    }

    // The libSBOLj3 jar embeds maven build metadata; fall back to the
    // implementation version on the package if the property file is
    // missing.
    private static String libsbolj3Version() {
        Package pkg = SBOLDocument.class.getPackage();
        if (pkg != null) {
            String v = pkg.getImplementationVersion();
            if (v != null && !v.isEmpty()) {
                return v;
            }
        }
        try (var input = SBOLDocument.class.getResourceAsStream(
                "/META-INF/maven/org.sbolstandard/libsbolj3/pom.properties")) {
            if (input != null) {
                Properties props = new Properties();
                props.load(input);
                String v = props.getProperty("version");
                if (v != null) {
                    return v;
                }
            }
        } catch (Exception ignored) {
            // fall through
        }
        return "unknown";
    }

    private static StringBuilder appendStringField(StringBuilder json, String name, String value) {
        json.append('"').append(escape(name)).append("\":\"").append(escape(value)).append('"');
        return json;
    }

    private static StringBuilder appendLongField(StringBuilder json, String name, long value) {
        json.append('"').append(escape(name)).append("\":").append(value);
        return json;
    }

    private static StringBuilder appendLongArrayField(StringBuilder json, String name, long[] values) {
        json.append('"').append(escape(name)).append("\":[");
        for (int i = 0; i < values.length; i++) {
            if (i > 0) {
                json.append(',');
            }
            json.append(values[i]);
        }
        json.append(']');
        return json;
    }

    private static String escape(String s) {
        StringBuilder out = new StringBuilder(s.length() + 8);
        for (int i = 0; i < s.length(); i++) {
            char c = s.charAt(i);
            switch (c) {
                case '\\':
                    out.append("\\\\");
                    break;
                case '"':
                    out.append("\\\"");
                    break;
                case '\n':
                    out.append("\\n");
                    break;
                case '\r':
                    out.append("\\r");
                    break;
                case '\t':
                    out.append("\\t");
                    break;
                default:
                    if (c < 0x20) {
                        out.append(String.format("\\u%04x", (int) c));
                    } else {
                        out.append(c);
                    }
            }
        }
        return out.toString();
    }
}
