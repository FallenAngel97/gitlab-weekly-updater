FROM node:22-alpine as build
WORKDIR /app
ADD . .
RUN yarn
FROM scratch as base
COPY --from=build . .
CMD ["node", "/app/script.js"]
