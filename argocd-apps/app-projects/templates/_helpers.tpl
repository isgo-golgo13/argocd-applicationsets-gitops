{{/*
---------------------------------------------------------------------------
Project naming.

With perEnvironment true, a project name is "<layer>-<env>" -- cluster-apps-prod.
With it false, the name is just "<layer>" and one project spans every cluster.

The ApplicationSets in ../app-sets/ derive the same names from their own
projects.perEnvironment flag. The two MUST agree; hack/validate.py enforces it.
---------------------------------------------------------------------------
*/}}

{{- define "proj.name" -}}
{{- $ctx := .ctx -}}
{{- if $ctx.Values.perEnvironment -}}
{{- printf "%s-%s" .base .env -}}
{{- else -}}
{{- .base -}}
{{- end -}}
{{- end -}}

{{- define "proj.labels" -}}
app.kubernetes.io/name: {{ .Chart.Name }}
app.kubernetes.io/instance: {{ .Release.Name }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
app.kubernetes.io/part-of: gitops-platform
{{- end -}}
