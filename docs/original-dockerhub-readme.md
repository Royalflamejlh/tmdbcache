# Movie DB

Web app to manage your video library. Movie information is retrieved from the TMDB web service and both images and movie information is cached locally. A TMDB API key is required. An optional keycloak integration can be configured to require a login. Web service utilization can be posted to an optional InfluxDB to visualize local REST and remote TMDB API usage.

- https://github.com/justsomebody42/movieDB
- https://hub.docker.com/r/moviedb/moviedb
- https://ko-fi.com/justsomebody42

## Running Movie DB

### Docker

Minimal configuration with docker compose. Replace `MOVIEDB_TMDB_APIKEY` with your API key.

```yaml
---
version: "2.0"
services:
  moviedb:
    image: moviedb/moviedb:1.0.14
    container_name: moviedb
    environment:
      - MOVIEDB_TMDB_APIKEY=...
      - MOVIEDB_KEYCLOAK_ENABLED=false
      - SPRING_PROFILES_ACTIVE=h2file
    restart: unless-stopped
    volumes:
      - "./database:/database"
      - "./imageCache/:/imageCache"
    ports:
      - "8081:8081"
```

### Podman

Minimal configuration for podman. Replace `MOVIEDB_TMDB_APIKEY` with your API key.

```bash
podman run -d --publish=8081:8081 -e="MOVIEDB_TMDB_APIKEY=..." -e="SPRING_PROFILES_ACTIVE=h2file" -e="MOVIEDB_KEYCLOAK_ENABLED=false" -v ./database:/database -v ./imageCache/:/imageCache docker.io/moviedb/moviedb:1.0.14
```

## Breaking changes

### Update from 1.0.x to 2.0.0

#### Renamed Columns

To support MySQL, the following columns had to be renamed. To upgrade from 1.0.x to 2.0.0, stop your instance and rename these columns in your existing h2 database or start with a fresh db.

- CAST_MEMBER.CHARACTER -> CAST_MEMBER.CAST_CHARACTER
- GUEST_STAR.CHARACTER -> GUEST_STAR.GUEST_STAR_CHARACTER

```sql
ALTER TABLE CAST_MEMBER ALTER COLUMN CHARACTER RENAME TO CAST_CHARACTER;
ALTER TABLE GUEST_STAR ALTER COLUMN CHARACTER RENAME TO GUEST_STAR_CHARACTER;
```

#### Required environment variable

`SPRING_PROFILES_ACTIVE` needs to be set to either `h2file` or `mysql` to chose desired database driver.
If `mysql` is chosen, MySQL specific variables as documented below need to be added to configure the database.

#### Changed environment variable

As movie and tv cast is now displayed separately, `MOVIEDB_NUMBER_OF_CAST_REFERENCES` and `MOVIEDB_SHOW_CAST` have been splitted into separate variables:

- MOVIEDB_SHOW_MOVIE_CAST
- MOVIEDB_NUMBER_OF_MOVIE_CAST_REFERENCES
- MOVIEDB_SHOW_TV_CAST
- MOVIEDB_NUMBER_OF_TV_CAST_REFERENCES

## Configuration

### Required environment variables

These variables need to be set, otherwise the app won't start:

- MOVIEDB_TMDB_APIKEY
- MOVIEDB_KEYCLOAK_ENABLED

### Optional environment variables

Use the following environment variables to configure the application:

- MOVIEDB_ADD_MEDIATYPE_HEADER_TO_VIDEOCARD (default: true)
- MOVIEDB_LOW_RATING_THRESHOLD (default: 40)
- MOVIEDB_HIGH_RATING_THRESHOLD (default: 70)
- MOVIEDB_USE_MOVIEBACKGROUNDS (default: true)
- MOVIEDB_SHOW_CAST (default: true)
- MOVIEDB_SHOW_RECOMMENDATIONS (default: true)
- MOVIEDB_LIST_MAX_CARDS (default: 200)
- MOVIEDB_LIST_MAX_LIGHT_CARDS (default: 300)
- MOVIEDB_NUMBER_OF_RECOMMENDATIONS (default: 12)
- MOVIEDB_NUMBER_OF_CAST_REFERENCES (default: 12)
- MOVIEDB_NUMBER_OF_DIRECTED_MOVIES (default: 12)
- MOVIEDB_SHOW_TVSHOWS_IN_VIDEOLIST (default: true)
- MOVIEDB_SHOW_TVSEASONS_IN_VIDEOLIST (default: true)
- MOVIEDB_DEFAULT_MOBILE_POSTERWIDTH (default: 133)
- MOVIEDB_DEFAULT_DESKTOP_POSTERWIDTH (default: 220)
- MOVIEDB_SUPPORT_DETAIL_CARDS (default: false)

### Wallpapers for the video list

You may add .jpg or .png files to the folder `wallpapers` in the mounted folder `imageCache`. The `wallpapers` folder will be created during the first start of the container, but you can create it before manually as well. The backend will watch for changes in the folder and images added to this folder will be available after ~8 seconds. The frontend needs to be refreshed to retrieve the updated configuration.

### MySQL DB

If MySQL is chosen as database backend, the following variables need to be set to configure the database connection

- MOVIEDB_DATABASE_HOST
- MOVIEDB_DATABASE_PORT
- MOVIEDB_DATABASE_NAME
- MOVIEDB_DATABASE_USERNAME
- MOVIEDB_DATABASE_PASSWORD

### Influx DB

WebService utilization can be added to an InfluxDB. To activate, provide the following environment variables.

- MOVIEDB_INFLUXDB_TOKEN
- MOVIEDB_INFLUXDB_ORG
- MOVIEDB_INFLUXDB_BUCKET (default: MovieDB)
- MOVIEDB_INFLUXDB_SERVER_URL

### Keycloak

Add keycloak configuration to enable login restrictions

- MOVIEDB_KEYCLOAK_ENABLED (required)
- MOVIEDB_KEYCLOAK_SERVER (default: '')
- MOVIEDB_KEYCLOAK_REALM
- MOVIEDB_KEYCLOAK_CLIENTID
- MOVIEDB_KEYCLOAK_REQUIRE_LOGIN (default: false)
- MOVIEDB_KEYCLOAK_ADMIN_ROLE (default: moviedb-admin)
- MOVIEDB_KEYCLOAK_USER_ROLE (default: moviedb-user)

### CA certificates

Connecting to the keycloak server might require a custom ca certificate to be added to the java keystore, if the keycloak instance is using a self-signed certificate.
This can be done manually or by the MovieDB application during startup, which could be helpful in container deployments.
To do so specify the .crt filename (full, absolute path. Reachable within the container) in the environment variable `MOVIEDB_CUSTOM_CA_CERT`. You can set `MOVIEDB_PRINT_CA_CERTS` to true, if you want to print all certificats present in the cacerts keystore for debugging.

- MOVIEDB_CUSTOM_CA_CERT
- MOVIEDB_PRINT_CA_CERTS (default: false)

### Emby

Add Emby configuration with the following environment variables. Emby updater is disabled, when `MOVIEDB_EMBY_BASEURL` is empty.

- MOVIEDB_EMBY_USERID
- MOVIEDB_EMBY_APIKEY
- MOVIEDB_EMBY_BASEURL (E.g. 'http://&lt;hostname&gt;:8096/emby/')

# Attribution

- Watchlist Ribbon:
  - https://www.svgrepo.com/svg/61436/bookmark
- Network error image:
  - https://wallpapersafari.com/w/vrYtf2

