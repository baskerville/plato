use anyhow::{Error, format_err};
use reqwest::blocking::Client;
use serde_json::Value as JsonValue;
use crate::app::Context;
use crate::helpers::{encode_entities, trim_non_alphanumeric};

pub fn translate(query: &str, source: &str, target: &str, context: &Context) -> Result<(String, String), Error> {

    let params = vec![
        ("client", "gtx"),
        ("ie", "UTF-8"),   // input encoding
        ("oe", "UTF-8"),   // output encoding
        ("sl", source),    // source language
        ("tl", target),    // target language
        ("dt", "t"),       // translation of source text
        ("dt", "at"),      // alternate translations
        ("dt", "md"),      // definitions of source text
        ("q", query),      // source text to translate
    ];
    let server = trim_non_alphanumeric(&context.settings.google_translate_server);
    let url = format!("{}/translate_a/single", server);
    let client = Client::new();

    let response = client.get(&url)
                         .query(&params)
                         .send()?;
    if !response.status().is_success() {
        return Err(format_err!("Unable to connect to {}: {}", server, response.status()));
    }

    let mut text = String::new();
    let body: JsonValue = response.json()?;
    let lang = body.get(2).unwrap().as_str().unwrap().to_string();

    if let Some(xlats) = body.get(0).and_then(JsonValue::as_array) {


        // translations are arrays of [source-sentence, translated-sentence]
        text.push_str("<p class='translated'><big>&#9635; </big>");
        for item in xlats {
            text.push_str(&encode_entities(&item[0].as_str().unwrap()));
        }
        text.push_str("<p class='original'><big>&#9669; </big>");
        text.push_str(&encode_entities(&query));
        text.push_str("</p>");

        if let Some(alts) = body.get(5).and_then(JsonValue::as_array) {
            text.push_str("<h3>Alternate translations</h3><dl>");

            // alternate translations are arrays of [source-sentence, array of translated sentences]
            for item in alts {
                text.push_str(&format!("<dt class='def'>{}</dt><dd><ul>",
                                       encode_entities(&item[0].as_str().unwrap())));
                for xlat in item.get(2).and_then(JsonValue::as_array).unwrap() {
                    text.push_str(&format!("<li>{}</li>",
                                           encode_entities(&xlat[0].as_str().unwrap())));
                }
                text.push_str("</ul></dd>");
            }
            text.push_str("</dl>");
        }

        if let Some(categories) = body.get(12).and_then(JsonValue::as_array) {

            // definitions are arrays of [category, array of defintitions]
            // where category = (noun | verb | adjective | etc)
            text.push_str("<h3>Definitions</h3><dl>");
            for cat in categories {
                text.push_str(&format!("<dt class='category'>{}</dt><dd><ul>",
                                       encode_entities(&cat[0].as_str().unwrap())));
                for def in cat.get(1).and_then(JsonValue::as_array).unwrap() {
                    text.push_str(&format!("<li>{}</li>",
                                           encode_entities(&def[0].as_str().unwrap())))

                }
                text.push_str("</ul></dd>");
            }
            text.push_str("</dl>");
        }
    }
    Ok((text, lang))
}

/*
Sample translate session
curl "https://translate.googleapis.com/translate_a/single?client=gtx&ie=UTF-8&oe=UTF-8&sl=auto&tl=en&dt=t&dt=at&dt=md&q=Je+demande+pardon+aux+enfants+d'avoir+dédié+ce+livre+à+une+grande+personne.+J'ai+une+excuse+sérieuse"

returned array

            0: 'translation',
            1: 'all-translations',
            2: 'original-language',
            5: 'possible-translations',
            6: 'confidence',
            7: 'possible-mistakes',
            8: 'language',
            11: 'synonyms',
            12: 'definitions',
            13: 'examples',
            14: 'see-also',

[
    [
        [
            "I apologize to the children for dedicating this book to a grown-up. ",
            "Je demande pardon aux enfants d'avoir dédié ce livre à une grande personne.",
            null,
            null,
            3,
            null,
            null,
            [
                []
            ],
            [
                [
                    [
                        "4df5d4d9d819b397555d03cedf085f48",
                        "fr_en_2022q1.md"
                    ]
                ]
            ]
        ],
        [
            "I have a serious excuse",
            "J'ai une excuse sérieuse",
            null,
            null,
            3,
            null,
            null,
            [
                []
            ],
            [
                [
                    [
                        "4df5d4d9d819b397555d03cedf085f48",
                        "fr_en_2022q1.md"
                    ]
                ]
            ]
        ]
    ],
    null,
    "fr",
    null,
    null,
    [
        [
            "Je demande pardon aux enfants d'avoir dédié ce livre à une grande personne.",
            null,
            [
                [
                    "I apologize to the children for dedicating this book to a grown-up.",
                    0,
                    true,
                    false,
                    [
                        3
                    ],
                    null,
                    [
                        [
                            3
                        ]
                    ]
                ],
                [
                    "I beg the children's forgiveness for dedicating this book to a grown-up.",
                    0,
                    true,
                    false,
                    [
                        8
                    ]
                ]
            ],
            [
                [
                    0,
                    75
                ]
            ],
            "Je demande pardon aux enfants d'avoir dédié ce livre à une grande personne.",
            0,
            0
        ],
        [
            "J'ai une excuse sérieuse",
            null,
            [
                [
                    "I have a serious excuse",
                    0,
                    true,
                    false,
                    [
                        3
                    ],
                    null,
                    [
                        [
                            3
                        ]
                    ]
                ],
                [
                    "i have a serious apology",
                    0,
                    true,
                    false,
                    [
                        8
                    ]
                ]
            ],
            [
                [
                    0,
                    24
                ]
            ],
            "J'ai une excuse sérieuse",
            0,
            0
        ]
    ],
    1,
    [],
    [
        [
            "fr"
        ],
        null,
        [
            1
        ],
        [
            "fr"
        ]
    ]
]
*/