# Manutenção

## Versões

A versão vive em três lugares que devem ser iguais: `package.json`,
`src-tauri/Cargo.toml` e `src-tauri/tauri.conf.json`. Ao abrir o ciclo seguinte
depois de publicar, derive-a da última tag estável (`X.Y.Z` vira
`X.Y.(Z+1)`); saltos minor ou major são decididos pelo mantenedor.

## Changelog

Toda mudança ou adição — código, documentação, testes, dependências,
empacotamento e workflows — entra na seção marcada `In development` de
`CHANGELOG.md`, agrupada em `Added`, `Changed`, `Fixed` ou `Removed`.

## Preferências persistidas

As preferências ficam em `settings.json` no diretório de configuração do app
(`$XDG_CONFIG_HOME/io.github.quantmind_br.fastzap/` no Linux). O arquivo tem um
campo `version`; mudanças de formato devem acrescentar uma etapa em
`settings::store::migrate` em vez de alterar o significado de um campo.

## Traduções

Ao atualizar `po/*.po`, rode `pnpm i18n` e versione os JSON gerados em
`src/lib/i18n/catalogs/`. Strings novas da UI usam o texto em inglês como
`msgid`.

## Empacotamento

Os artefatos e manifestos ficam em `packaging/`; os workflows em
`.github/workflows/`. Veja [arquitetura](architecture.md#empacotamento).
