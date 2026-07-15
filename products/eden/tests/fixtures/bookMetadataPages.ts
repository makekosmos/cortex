import type { BookMetadataPage } from "../../src/lib/bookMetadata";

export const LIVELIB_LIKE_BOOK_PAGE: BookMetadataPage = {
  finalUrl: "https://www.livelib.ru/book/1000000001-marafon-v-raj-artur-klark",
  html: `<!doctype html>
<html lang="ru">
  <head>
    <meta property="og:image" content="/storage/covers/marafon-v-raj.jpg">
  </head>
  <body>
    <main>
      <h1>Марафон в рай</h1>
      <a rel="author">Артур Кларк</a>
      <dl>
        <dt>ISBN</dt><dd>978-0-306-40615-7</dd>
        <dt>Количество страниц</dt><dd>352</dd>
        <dt>Язык</dt><dd>Русский</dd>
        <dt>Издательство</dt><dd>АСТ</dd>
        <dt>Год издания</dt><dd>2024</dd>
      </dl>
    </main>
  </body>
</html>`,
};

export const SCHEMA_ORG_BOOK_PAGE: BookMetadataPage = {
  finalUrl: "https://books.example/library/solaris",
  html: `<!doctype html>
<html lang="pl">
  <head>
    <script type="application/ld+json">
      {
        "@context": "https://schema.org",
        "@type": "Book",
        "name": "Solaris",
        "author": { "@type": "Person", "name": "Stanisław Lem" },
        "image": { "url": "/covers/solaris.jpg" },
        "isbn": "978-0-306-40615-7",
        "numberOfPages": 224,
        "inLanguage": "pl",
        "publisher": { "@type": "Organization", "name": "Wydawnictwo Literackie" },
        "datePublished": "1961"
      }
    </script>
  </head>
  <body><h1>Fallback title that must not win</h1></body>
</html>`,
};
