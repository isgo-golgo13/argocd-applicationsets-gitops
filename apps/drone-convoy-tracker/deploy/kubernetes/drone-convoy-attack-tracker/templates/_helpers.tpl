{{/* Standard names and labels. */}}
{{- define "drone.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" -}}
{{- end -}}

{{- define "drone.fullname" -}}
{{- printf "%s" (include "drone.name" .) -}}
{{- end -}}

{{- define "drone.labels" -}}
app.kubernetes.io/name: {{ include "drone.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
helm.sh/chart: {{ printf "%s-%s" .Chart.Name .Chart.Version }}
{{- end -}}

{{- define "drone.selector.api" -}}
app.kubernetes.io/name: {{ include "drone.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
app.kubernetes.io/component: api
{{- end -}}

{{- define "drone.selector.frontend" -}}
app.kubernetes.io/name: {{ include "drone.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
app.kubernetes.io/component: frontend
{{- end -}}

{{- define "drone.image.api" -}}
{{- printf "%s/%s:%s" .Values.image.registry .Values.image.api.repository (default .Chart.AppVersion .Values.image.api.tag) -}}
{{- end -}}

{{- define "drone.image.frontend" -}}
{{- printf "%s/%s:%s" .Values.image.registry .Values.image.frontend.repository (default .Chart.AppVersion .Values.image.frontend.tag) -}}
{{- end -}}

{{/* Gateway namespace: explicit or the release namespace. */}}
{{- define "drone.gateway.namespace" -}}
{{- default .Release.Namespace .Values.gateway.namespace -}}
{{- end -}}

{{/* ScyllaDB contact points: in-chart cluster's client Service, or the given hosts. */}}
{{- define "drone.scylla.hosts" -}}
{{- if .Values.scylla.create -}}
{{- printf "%s-scylla-client.%s.svc:9042" (include "drone.fullname" .) .Release.Namespace -}}
{{- else -}}
{{- .Values.scylla.hosts -}}
{{- end -}}
{{- end -}}

{{- define "drone.redis.url" -}}
{{- if .Values.redis.create -}}
{{- printf "redis://%s-redis.%s.svc:6379" (include "drone.fullname" .) .Release.Namespace -}}
{{- else -}}
{{- .Values.redis.url -}}
{{- end -}}
{{- end -}}

{{- define "drone.tls.secret" -}}
{{- printf "%s-tls" (include "drone.fullname" .) -}}
{{- end -}}
