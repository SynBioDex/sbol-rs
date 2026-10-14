import java.io.*;
import java.nio.charset.StandardCharsets;
import java.nio.file.*;
import java.security.MessageDigest;
import java.util.*;
import org.apache.jena.rdf.model.*;
import org.apache.jena.rdf.model.Model;
import org.sbolstandard.core3.entity.*;
import org.sbolstandard.core3.io.*;
import org.sbolstandard.core3.util.*;

public class Matched {
    static final String S = "http://sbols.org/v3#";
    static final String TYPE = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";
    static void row(List<String> rows, String id, String predicate, String kind, Object value) {
        rows.add(id+"\t"+predicate+"\t"+kind+"\t"+Objects.requireNonNull(value)+"\n");
    }
    static void prop(List<String> rows, Identified obj, String name, String kind, Object value) {
        row(rows, obj.getUri().toString(), S+name, kind, value);
    }
    static void visit(List<String> rows, Identified obj) throws Exception {
        row(rows, obj.getUri().toString(), TYPE, "I", obj.getResourceType());
        prop(rows, obj, "displayId", "S", obj.getDisplayId());
        if (obj instanceof TopLevel top) prop(rows, obj, "hasNamespace", "I", top.getNamespace());
        if (obj instanceof Component c) {
            for (var v : c.getTypes()) prop(rows, c, "type", "I", v);
            for (var v : c.getRoles()) prop(rows, c, "role", "I", v);
            for (var v : c.getSequenceURIs()) prop(rows, c, "hasSequence", "I", v);
            for (var child : c.getFeatures()) {
                prop(rows, c, "hasFeature", "I", child.getUri()); visit(rows, child);
            }
        } else if (obj instanceof Sequence c) {
            prop(rows, c, "elements", "S", c.getElements());
            prop(rows, c, "encoding", "I", c.getEncoding());
        } else if (obj instanceof SubComponent c) {
            prop(rows, c, "instanceOf", "I", c.getInstanceOfURI());
            prop(rows, c, "orientation", "I", c.getOrientation().getUri());
            for (var child : c.getLocations()) {
                prop(rows, c, "hasLocation", "I", child.getUri()); visit(rows, child);
            }
        } else if (obj instanceof Range c) {
            prop(rows, c, "hasSequence", "I", c.getSequenceURI());
            prop(rows, c, "orientation", "I", c.getOrientation().getUri());
            prop(rows, c, "start", "N", c.getStart().orElseThrow());
            prop(rows, c, "end", "N", c.getEnd().orElseThrow());
        } else throw new IllegalStateException("unexpected class: "+obj.getClass());
    }
    static List<String> snapshot(SBOLDocument doc) throws Exception {
        List<String> rows = new ArrayList<>();
        if (doc.getComponents()!=null) for (var c : doc.getComponents()) visit(rows, c);
        if (doc.getSequences()!=null) for (var c : doc.getSequences()) visit(rows, c);
        return rows;
    }
    static List<String> rdfRows(Model model) {
        List<String> rows = new ArrayList<>();
        StmtIterator it = model.listStatements();
        try {
            while (it.hasNext()) {
                var t = it.next(); var o = t.getObject();
                String kind = o.isURIResource() ? "I" :
                    "http://www.w3.org/2001/XMLSchema#integer".equals(o.asLiteral().getDatatypeURI()) ? "N" : "S";
                row(rows, t.getSubject().getURI(), t.getPredicate().getURI(), kind,
                    o.isURIResource() ? o.asResource().getURI() : o.asLiteral().getLexicalForm());
            }
        } finally { it.close(); }
        return rows;
    }
    static String fingerprint(List<String> rows) throws Exception {
        Collections.sort(rows);
        MessageDigest digest = MessageDigest.getInstance("SHA-256");
        for (String row : rows) digest.update(row.getBytes(StandardCharsets.UTF_8));
        return HexFormat.of().formatHex(digest.digest());
    }
    public static void main(String[] a) throws Exception {
        var config = Configuration.getInstance();
        config.setValidateAfterSettingProperties(false); config.setValidateAfterReadingSBOLDocuments(false);
        config.setValidateBeforeSaving(false); config.setValidateRecommendedRules(true); config.setCompleteDocument(true);
        byte[] input = Files.readAllBytes(Path.of(a[0])); String mode = a[1];
        int warm = Integer.parseInt(a[2]), iters = Integer.parseInt(a[3]);
        List<Long> times = new ArrayList<>(), serial = new ArrayList<>();
        List<String> hashes = new ArrayList<>(); List<Integer> counts = new ArrayList<>(); int lastBytes = 0;
        for (int i=0; i<warm+iters; i++) {
            long start = System.nanoTime(), elapsed, serialized = 0; List<String> rows;
            if (mode.equals("rdf")) {
                Model graph = RDFUtil.read(new ByteArrayInputStream(input), SBOLFormat.RDFXML.getFormat());
                elapsed = System.nanoTime()-start; rows = rdfRows(graph); graph.close();
            } else {
                SBOLDocument doc = SBOLIO.read(new ByteArrayInputStream(input), SBOLFormat.RDFXML);
                rows = snapshot(doc); elapsed = System.nanoTime()-start;
                start = System.nanoTime(); ByteArrayOutputStream out = new ByteArrayOutputStream();
                SBOLIO.write(doc, out, SBOLFormat.RDFXML); serialized = System.nanoTime()-start;
                lastBytes = out.size(); doc.getRDFModel().close();
            }
            String hash = fingerprint(rows);
            if (!hash.equals(a[5])) throw new IllegalStateException("endpoint hash "+hash+" != "+a[5]);
            if (i>=warm) { times.add(elapsed); serial.add(serialized); hashes.add("\""+hash+"\""); counts.add(rows.size()); }
            System.err.printf("completed %s iteration %d/%d%n",mode,i+1,warm+iters);
        }
        String json = "{\"implementation\":\"java\",\"mode\":\""+mode+"\",\"warmup\":"+warm+
            ",\"iterations\":"+iters+",\"time_ns\":"+times+",\"serialize_ns\":"+serial+
            ",\"fingerprints\":"+hashes+",\"row_counts\":"+counts+",\"serialized_bytes\":"+lastBytes+"}";
        Files.writeString(Path.of(a[4]), json);
    }
}
