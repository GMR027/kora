// Datos de la demostración en navegador. No se cargan dentro de Tauri: el
// módulo solo se importa cuando no existe `window.__TAURI_INTERNALS__` en
// desarrollo. Todo es ficticio, pero los libros, capítulos, metadatos y
// portadas imitan el catálogo de una biblioteca real.

import type {
  AccountView,
  BookSummary,
  Category,
  Locator,
  ReaderSettingsInput,
} from "../types";

export interface DemoChapter {
  title: string;
  paragraphs: string[];
}

type CoverMotif =
  | "waves"
  | "sun"
  | "arch"
  | "leaves"
  | "grid"
  | "dots"
  | "moon"
  | "lines";

export interface DemoBookSeed {
  id: string;
  title: string;
  author: string;
  format: "epub" | "pdf";
  description: string;
  publisher: string;
  language: string;
  sizeBytes: number;
  addedAt: string;
  lastReadAt: string | null;
  progress: number;
  locator: Locator;
  favorite: boolean;
  readOnly: boolean;
  categoryIds: string[];
  palette: [string, string, string];
  motif: CoverMotif;
  audiobook?: boolean;
  settings?: Partial<ReaderSettingsInput>;
  chapters?: DemoChapter[];
  pdfPages?: string[][];
}

export const demoAccount: AccountView = {
  id: "account-elena",
  username: "elena",
  displayName: "Elena Marín",
  publicKey: "kora-demo-public-key",
  hasPin: true,
  connectionKeyAvailable: true,
  connectionKeyRotatedAt: "2026-11-12T18:20:00Z",
  avatarPath: null,
  createdAt: "2025-11-03T09:30:00Z",
};

export const demoCategories: Category[] = [
  {
    id: "cat-novela",
    name: "Novela",
    color: "#b45309",
    sortOrder: 0,
    createdAt: "2025-11-03T09:31:00Z",
    updatedAt: "2025-11-03T09:31:00Z",
    ownerAccountId: demoAccount.id,
    originShareId: null,
  },
  {
    id: "cat-ensayo",
    name: "Ensayo",
    color: "#3f6b4f",
    sortOrder: 1,
    createdAt: "2025-11-03T09:31:00Z",
    updatedAt: "2025-11-03T09:31:00Z",
    ownerAccountId: demoAccount.id,
    originShareId: null,
  },
  {
    id: "cat-poesia",
    name: "Poesía",
    color: "#7c3aed",
    sortOrder: 2,
    createdAt: "2025-11-03T09:31:00Z",
    updatedAt: "2025-11-03T09:31:00Z",
    ownerAccountId: demoAccount.id,
    originShareId: null,
  },
  {
    id: "cat-ciencia",
    name: "Ciencia ficción",
    color: "#2563eb",
    sortOrder: 3,
    createdAt: "2025-11-03T09:31:00Z",
    updatedAt: "2025-11-03T09:31:00Z",
    ownerAccountId: demoAccount.id,
    originShareId: null,
  },
  {
    id: "cat-historia",
    name: "Historia",
    color: "#92722a",
    sortOrder: 4,
    createdAt: "2025-11-03T09:31:00Z",
    updatedAt: "2025-11-03T09:31:00Z",
    ownerAccountId: demoAccount.id,
    originShareId: null,
  },
  {
    id: "cat-club",
    name: "Club de lectura",
    color: "#be123c",
    sortOrder: 5,
    createdAt: "2026-01-14T19:05:00Z",
    updatedAt: "2026-01-14T19:05:00Z",
    ownerAccountId: demoAccount.id,
    originShareId: null,
  },
];

const chapter = (title: string, ...paragraphs: string[]): DemoChapter => ({
  title,
  paragraphs,
});

const mareas: DemoChapter[] = [
  chapter(
    "La casa junto al mar",
    "Aquella mañana el viento soplaba con una fuerza inusual. Elena cerró la puerta de la casa y caminó hacia el acantilado, donde las olas golpeaban las rocas con un ruido sordo y constante.",
    "Recordó las palabras de su abuelo: «El mar nunca olvida a quienes lo escuchan». Desde niña había creído que las historias viajaban con la marea, y que cada invierno traía de vuelta a los ahogados en forma de bruma.",
    "En el pueblo todos conocían su nombre, pero nadie se atrevía a pronunciarlo delante del farero. La luz del faro giraba cada noche, y con ella giraban los secretos de una familia que llevaba tres generaciones callando lo mismo.",
    "—No deberías haber vuelto —dijo una voz a su espalda.",
    "Elena se giró despacio. El hombre llevaba un abrigo gris y sostenía una linterna apagada. Su rostro era el de alguien que había esperado demasiado tiempo.",
    "—Tenía que saber la verdad —respondió ella—. Aunque la verdad sea lo que se llevó a mi madre.",
    "El farero bajó la mirada. El viento arrastraba sal y pequeñas gotas de agua que brillaban como cristales rotos bajo el cielo blanco.",
    "—Entonces ven conmigo —dijo—. La marea está bajando, y la puerta solo aparece una vez cada veinte años.",
    "Caminaron en silencio por el sendero de piedra. A lo lejos, la casa parecía respirar, con sus ventanas oscuras y su tejado cubierto de musgo. Elena sintió que alguien la observaba desde dentro.",
    "Cuando llegaron a la cueva, el agua ya se había retirado. Una puerta de hierro emergía de la arena, cubierta de percebes y de runas que ella nunca había visto antes.",
    "El farero sacó una llave del bolsillo y la sostuvo en alto. La llave brilló, y por un instante el mar entero pareció quedarse quieto.",
    "—Lo que hay detrás no se puede devolver —advirtió—. ¿Sigues queriendo entrar?",
    "Elena extendió la mano sin responder. La llave estaba fría, más fría que el agua, y pesaba como un recuerdo que se niega a desaparecer.",
  ),
  chapter(
    "El cuarto de las mareas",
    "La puerta se abrió con un gemido largo. Dentro, el aire olía a sal y a papel viejo. Las paredes estaban cubiertas de mapas dibujados a mano, con líneas que no correspondían a ninguna costa conocida.",
    "Elena encendió la linterna y avanzó con cuidado. En el centro del cuarto había una mesa de madera y, sobre ella, un libro abierto por la mitad. Las páginas estaban en blanco.",
    "—Mi madre escribió aquí —dijo, reconociendo la letra menuda y apretada—. Estas son sus notas.",
    "El farero asintió. —Durante años vino cada noche. Decía que el mar le hablaba y que necesitaba escribir lo que oía.",
    "Elena pasó una página. Un dibujo de una mujer con un farol ocupaba toda la hoja. Debajo, en tinta roja, alguien había escrito una fecha: el día en que su madre desapareció.",
    "—¿Qué significa? —preguntó.",
    "—Que ella también encontró la puerta. Y que no pudo volver.",
    "Un ruido metálico resonó al fondo del cuarto. Elena levantó la cabeza y vio una segunda puerta, más pequeña, iluminada por una luz azul que parpadeaba como una respiración.",
    "—Hay algo vivo aquí —susurró.",
    "—No exactamente —respondió el farero—. Pero tampoco está muerto.",
    "Avanzaron hacia la puerta. El suelo estaba húmedo y cada paso dejaba una huella que se borraba a sus espaldas. Elena pensó en su abuelo, en las historias que le contaba de niña, y entendió que todas ellas habían sido una advertencia.",
    "Al otro lado de la puerta, el mar brillaba bajo un cielo sin sol.",
  ),
  chapter(
    "Lo que devuelve la marea",
    "El agua estaba quieta como un espejo. No había olas, ni viento, ni gaviotas. Solo una extensión infinita de luz que parecía respirar al ritmo de la linterna.",
    "Elena caminó hasta la orilla. Allí, sentada en una roca que no existía en ningún mapa, estaba su madre. Tenía el mismo abrigo azul que llevaba la última vez que la vio.",
    "—Llegas tarde —dijo su madre, sin sorpresa—. Te he estado esperando.",
    "Elena quiso correr hacia ella, pero sus pies se detuvieron. Era su madre, sí, pero algo en sus ojos no era humano. Eran ojos que habían visto demasiado mar.",
    "—¿Por qué te fuiste? —preguntó.",
    "—Porque tenía que saber si las historias eran verdad. Y lo son, Elena. Todas lo son.",
    "Su madre se levantó y señaló el horizonte. Allí, donde el cielo se unía con el agua, flotaban cientos de faroles encendidos. Cada uno llevaba un nombre escrito en la luz.",
    "—Son todos los que escucharon al mar —explicó—. Ahora les toca a ellos volver.",
    "Elena sintió una mano en el hombro. El farero estaba detrás de ella, con la mirada fija en los faroles.",
    "—Tienes que elegir —dijo—. Puedes quedarte aquí con ella, o volver y contar lo que has visto.",
    "El silencio se llenó de sal. Elena miró a su madre, luego al camino que había dejado atrás, y comprendió que ninguna de las dos opciones era gratuita.",
    "—Volveré —dijo—. Pero no para callar.",
    "Su madre sonrió. Era una sonrisa triste, como la de alguien que ya conoce el final de la historia.",
    "—Entonces corre —susurró—. La marea no espera a nadie.",
  ),
  chapter(
    "La costa de los nombres",
    "Al volver, el pueblo ya no parecía el mismo. Las casas se habían encogido y las calles olían a leña mojada. Elena caminó hasta el registro civil con la libreta de su madre apretada contra el pecho.",
    "La archivera era una mujer mayor que hablaba despacio, como si cada palabra costara una moneda. Le dijo que los libros de nacimientos de aquel siglo se habían perdido en un incendio.",
    "—No todos —respondió Elena—. Mi madre encontró una lista. Una lista de personas que no aparecen en ninguna parte.",
    "La mujer se quedó quieta. Por primera vez desde que había entrado, sus ojos dejaron de mirar la puerta.",
    "—Tu madre hacía preguntas que no se deben hacer en este pueblo.",
    "—Mi madre hacía las preguntas que ustedes dejaron de hacer.",
    "En el sótano, entre cajas de expedientes y olor a humedad, encontraron un armario metálico. Dentro había docenas de sobres con nombres de mujeres. Todos tenían la misma fecha de desaparición.",
    "Elena abrió el primero. La letra era de su madre. Debajo del nombre, una sola frase: «La marea devuelve lo que el pueblo entierra».",
    "Cuando salió del registro empezaba a llover. El faro, a lo lejos, seguía encendido. Elena supo entonces que la historia no había terminado: solo había cambiado de orilla.",
  ),
  chapter(
    "El faro encendido",
    "El farero la esperaba en la puerta del faro. No pareció sorprenderse al ver los sobres. Solo apartó la cortina y la dejó pasar.",
    "—Eran treinta y dos —dijo Elena—. Treinta y dos mujeres en cuarenta años.",
    "—Treinta y tres —corrigió él—. La última fuiste tú, aunque no llegaste a cruzar.",
    "La cólera le subió a la garganta como agua hirviendo. Quiso gritar, quiso romper algo, quiso correr hasta el acantilado y tirar la llave al mar.",
    "En lugar de eso, preguntó: —¿Por qué las elegían?",
    "—No las elegían. Se ofrecían. El faro necesita una voz que lo mantenga despierto. Si se apaga, las historias se pierden.",
    "—Eso es una mentira muy bonita.",
    "—Puede ser. Pero es la mentira con la que hemos vivido todos.",
    "Elena subió los ciento doce escalones hasta la sala de la linterna. Fuera, el mar era una sábana negra. Colocó los sobres sobre la mesa y encendió el faro con mano firme.",
    "La luz barrió la costa, las casas, el muelle vacío. Por un instante, Elena creyó ver a su madre en el agua, levantando la mano. No sintió miedo. Sintió una especie de paz antigua, como si por fin entendiera su lugar en la historia.",
    "Al amanecer, bajó al pueblo y dejó los sobres en la puerta de la escuela. Luego escribió una carta al periódico provincial. La firmó con su nombre y con el de todas las mujeres de la lista.",
  ),
];

const diasDeSal: DemoChapter[] = [
  chapter(
    "El verano en que aprendimos a callar",
    "Aquel verano el pueblo amaneció cubierto de sal. No era una forma de hablar: el viento había arrastrado la espuma del temporal y la había dejado sobre los tejados, los geranios y las sillas de la plaza.",
    "Inés tenía doce años y una costumbre que su madre detestaba: preguntar por todo. ¿Por qué las barcas volvían vacías? ¿Por qué los hombres hablaban en voz baja en el muelle? ¿Por qué nadie decía el nombre de su tía?",
    "Su padre, Ramón, salía antes del alba y volvía cuando el sol ya se había escondido detrás del faro. Nunca contestaba las preguntas de Inés. Se limitaba a secarse las manos en el pantalón y a mirar el horizonte como si allí hubiera una respuesta escrita.",
    "Una mañana la red apareció vacía. No había ni un pez, ni un cangrejo, ni un trozo de alga. Ramón la levantó del suelo y la sostuvo en alto; el agua caía por los nudos como una cortina transparente.",
    "—Este año el mar no quiere nada con nosotros —dijo Damián, el hermano mayor.",
    "Nadie respondió. Aquel fue el primer día del silencio que duraría tres meses.",
  ),
  chapter(
    "Las redes vacías",
    "Pasaron las semanas y las redes siguieron vacías. El pueblo empezó a comer conservas y a hablar de marcharse. En la taberna, los hombres dibujaban mapas sobre las mesas con el dedo mojado en vino.",
    "Damián empezó a acompañar a su padre. Volvía con las manos llenas de sal y los ojos cansados, pero nunca contaba nada. Inés lo esperaba en la puerta y él pasaba a su lado sin mirarla.",
    "El muelle olía a gasóleo y a pescado viejo. Las gaviotas se peleaban por las vísceras que nadie se molestaba en tirar al agua. Inés se sentaba en el espigón y contaba los barcos que no volvían.",
    "Una tarde encontraron un mensaje dentro de una botella. Estaba escrito en un papel que se deshacía entre los dedos: «No vengáis a buscarme. El mar es más grande de lo que dicen los mapas».",
    "Inés quiso enseñárselo a su madre. Damián se lo quitó de las manos y lo guardó en el bolsillo.",
    "—No era para nosotros —dijo.",
    "Desde entonces, el silencio dejó de ser una ausencia y se convirtió en una forma de hablar.",
  ),
  chapter(
    "Un lugar en el muelle",
    "El otoño llegó con lluvia y con una carta del ayuntamiento. Las ayudas para los pescadores se habían agotado. Ramón firmó el papel sin leerlo y lo dejó sobre la mesa, junto al cenicero.",
    "Inés decidió quedarse. Damián se marchó a la capital en el primer autobús de la mañana. Antes de subir, le dio un abrazo tan rápido que ella no tuvo tiempo de devolverlo.",
    "Las cartas de Damián llegaban cada dos semanas. Hablaba de un trabajo en un almacén, de una habitación con vistas a un patio, de una ciudad que nunca se quedaba quieta. Nunca preguntaba por el mar.",
    "Su padre le enseñó a reparar el motor de la barca. Le explicó cada pieza con una paciencia que no había tenido antes. Inés aprendió que las manos también pueden decir lo que la boca calla.",
    "La casa siguió en pie, aunque la pintura se desconchaba y el tejado pedía un arreglo que nadie podía pagar. En el muelle, los barcos seguían saliendo. Inés los miraba desde la ventana y pensaba en las redes vacías, en la botella, en su hermano.",
    "Un día volvió a llenarse la red. No fue un milagro, ni una señal: solo el mar, que a veces recuerda a quienes se quedan.",
  ),
];

const cartasDelFaro: DemoChapter[] = [
  chapter(
    "Carta primera: la luz de octubre",
    "Querida Marta: hoy he vuelto al faro después de siete años. La puerta sigue sin cerrar bien y la escalera huele a sal, igual que entonces. Me he sentado en el último escalón y he esperado a que anocheciera, como si tú fueras a bajar de la linterna con el pelo mojado y una excusa en los labios.",
    "No has venido, claro. No sé por qué sigo escribiéndote cartas que nunca envío. Quizá porque el faro solo funciona si alguien le cuenta lo que ve.",
    "Desde aquí arriba la costa parece una línea mal trazada. Las casas del pueblo son puntos de luz y el mar, una sábana que respira. Anoche vi un barco que no debería existir: navegaba sin luces, contra la corriente, y desapareció detrás de la isla.",
    "He pensado en la primera vez que subimos juntas. Tú contabas los escalones en voz alta y yo te seguía con miedo de que te cansaras. Al llegar arriba, me dijiste que desde allí todo parecía posible.",
    "Tenías razón. Lo malo es que «todo» también incluye las cosas que no volverán.",
  ),
  chapter(
    "Carta segunda: lo que no dije en el muelle",
    "Querida Marta: en el muelle han puesto una placa con los nombres de los que se perdieron en el temporal del noventa y cuatro. El de mi padre está al final, casi borrado. He pasado la tarde limpiándolo con un paño y agua dulce.",
    "La gente del pueblo me saluda con la cabeza y sigue caminando. Nadie me pregunta por qué he vuelto. Supongo que aquí las preguntas se guardan para el invierno, cuando no hay faena y sobra tiempo.",
    "La mujer del estanco me ha dicho que tú vives en el norte, que tienes una casa con jardín y una hija que se llama como yo. No he sabido qué contestar. He comprado tabaco aunque no fumo.",
    "Hay algo que no te dije en el muelle la última vez que nos vimos. Lo dejé dentro del abrigo, junto a la carta que no me atreví a darte. Si alguna vez vuelves, lo encontrarás. Si no vuelves, ya no importa.",
    "El faro sigue encendido. Esta noche la luz tarda un segundo más en girar. Lo sé porque llevo contando los segundos desde que llegué.",
  ),
  chapter(
    "Carta tercera: cuando vuelvas",
    "Querida Marta: he decidido quedarme. No para esperarte, aunque al principio me lo dijera a mí misma. Me quedo porque el faro necesita a alguien y porque yo necesito un lugar donde las cosas tarden en cambiar.",
    "He pintado la cocina de blanco. He puesto una mesa junto a la ventana y una silla de más, por si acaso. No te escribo esto para que vengas. Te lo escribo para que sepas que hay una silla, y que no me la voy a llevar a ninguna parte.",
    "El invierno ha sido duro. Las tormentas han arrancado las tejas del cobertizo y el mar ha llegado hasta la puerta. Pero el faro ha aguantado. Los faros están hechos para eso: para seguir señalando aunque nadie mire.",
    "Ayer llegó una carta tuya. La he dejado sobre la mesa sin abrir. No por rencor, sino porque quiero elegir el momento. Cuando la lea, puede que todo cambie. Cuando la lea, puede que no cambie nada.",
    "Esta noche la luz ha vuelto a girar a su hora. Desde aquí arriba, la costa parece una línea mal trazada, y sin embargo sé exactamente dónde estoy.",
  ),
];

const mecanicaDelSilencio: DemoChapter[] = [
  chapter(
    "El taller de los relojes quietos",
    "En Varena la gente medía el tiempo con campanas. Había una para los nacimientos, otra para las bodas y una tercera, más grave, para los entierros. Los relojes de pared eran un lujo de ricos y de enfermos.",
    "Ilan heredó el taller de su maestro una mañana de febrero. Encontró la puerta abierta, la lámpara encendida y una taza de café todavía tibia. Sobre el mostrador había un reloj sin agujas y una nota con dos palabras: «Ya falta menos».",
    "Los vecinos decían que el maestro había pasado los últimos años intentando detener el tiempo. No en sentido figurado: quería construir una máquina capaz de sostener un instante y devolverlo intacto.",
    "Ilan no creía en máquinas imposibles. Creía en tornillos, en muelles, en la paciencia de las manos. Pero aquella nota le recordó que su maestro también había creído en los milagros.",
    "La primera vez que abrió el reloj sin agujas, encontró un compartimento oculto. Dentro había una llave diminuta y un papel doblado con una fecha: el día en que Ilan había nacido.",
    "Fuera empezó a nevar. En Varena, la nieve siempre llegaba tarde, como si hubiera perdido el rumbo.",
  ),
  chapter(
    "Una máquina para oír el pasado",
    "La llave abría una puerta del sótano que Ilan nunca había visto. Detrás había un taller más viejo, con estanterías llenas de relojes parados a distintas horas. Todos marcaban las tres y diecisiete.",
    "En el centro de la habitación había una máquina de latón con forma de caracol. Tenía cientos de engranajes y un único botón. Al pulsarlo, se oyó una voz: la de su madre, muerta cuando él tenía seis años, cantando una nana.",
    "Ilan retrocedió hasta la pared. La voz siguió cantando, ajena al tiempo, como si estuviera grabada en el aire y no en un mecanismo.",
    "En el cuaderno del maestro había una explicación. La máquina no viajaba al pasado. Lo que hacía era abrir una rendija en la memoria de las cosas. Las paredes, los relojes, las tazas: todo guardaba un eco de lo que había ocurrido.",
    "Las anotaciones se volvían más confusas al final. «No es el tiempo lo que se detiene», escribió el maestro. «Somos nosotros los que dejamos de escuchar».",
    "Ilan pasó la noche en el sótano, pulsando el botón. Escuchó la voz de su madre, la de su maestro, la de la mujer que vendía pan. Ninguna era exactamente como la recordaba. Todas eran mejores.",
  ),
  chapter(
    "La última cuerda",
    "Durante semanas Ilan no abrió el taller. Vivía de las reservas y de la luz que entraba por la ventana. La máquina seguía en el sótano, esperando.",
    "Un día llamó a la puerta una niña con un reloj roto. Tenía prisa porque su padre volvía de un viaje y quería darle la hora exacta. Ilan abrió el reloj, cambió una pieza y se lo devolvió. La niña sonrió y se fue corriendo.",
    "Esa noche Ilan entendió algo que el maestro no había escrito. La memoria no era un lugar al que volver, sino una cuerda que se tensa entre dos personas. Si una se suelta, la otra sigue vibrando.",
    "Subió al taller y abrió la vitrina de los relojes. Uno por uno, fue dándoles cuerda. Las agujas empezaron a moverse, lentas y tercas, como si nada hubiera pasado.",
    "Cuando llegó al reloj sin agujas, se detuvo. Lo sostuvo un momento en la mano y luego lo dejó en el escaparate. No hacía falta saber la hora para recordar lo importante.",
    "La nieve seguía cayendo sobre Varena. En la puerta del taller, alguien había dejado una taza de café. Estaba tibia.",
  ),
];

const jardinDeLasHoras: DemoChapter[] = [
  chapter(
    "La casa de las buganvillas",
    "La casa de mi abuela tenía un jardín que parecía no terminar nunca. Las buganvillas trepaban por la pared hasta la ventana del desván, y cada primavera se caían sobre el suelo como si alguien hubiera volcado un carro de papel de seda.",
    "Mi abuela se levantaba a las seis, regaba las macetas y preparaba café para dos. Nunca hablaba durante el desayuno. Decía que las palabras, si se decían demasiado pronto, se marchitaban.",
    "Yo pasaba allí los veranos. Aprendí a reconocer el canto de los pájaros, a distinguir la tierra seca de la húmeda, a esperar. Sobre todo, aprendí a esperar.",
    "Por la tarde nos sentábamos bajo la parra. Ella cosía y yo leía. A veces se quedaba dormida con la aguja en la mano, y el hilo subía y bajaba al ritmo de su respiración.",
    "Un día le pregunté por qué no vendía la casa. Era demasiado grande para ella, decían las vecinas. Ella me miró por encima de las gafas y respondió: —Porque las casas también crían raíces.",
    "Ahora entiendo que no hablaba de la casa.",
  ),
  chapter(
    "El cuaderno de las siestas",
    "En el cajón de la mesilla encontré un cuaderno cuando ya no quedaba nadie para impedírmelo. La portada estaba forrada con una tela azul y el lomo se deshacía entre los dedos.",
    "Dentro había recetas, cuentas del mercado y frases sueltas. «El tiempo de las siestas es el único que no se pierde», decía una. «Las mejores conversaciones ocurren cuando nadie está hablando», decía otra.",
    "También había cartas que nunca envió. Una para su madre, una para una hermana que se fue a Argentina, una para mi padre cuando era niño. Ninguna tenía sello.",
    "Me senté en la cocina y las leí todas. Al terminar, la casa parecía más grande y más silenciosa. El reloj de la pared seguía marcando las cuatro, como si se hubiera quedado dormido con ella.",
    "En la última página había una lista de cosas que quería hacer. Regar el limonero, aprender a hacer pan, ver el mar en invierno. Tachadas, casi todas. Al final, con letra temblorosa: «Decirle a Marta que el jardín es suyo».",
    "Marta era yo.",
  ),
  chapter(
    "Un invierno sin prisa",
    "No vendí la casa. Pinté la fachada, arreglé la puerta y planté dos buganvillas nuevas junto a la ventana del desván. La vieja siguió trepando por su cuenta, como una abuela terca.",
    "El primer invierno fue difícil. La caldera se estropeó tres veces y el pueblo se quedó sin luz durante dos días. Aprendí a leer con velas y a calentar agua en la estufa.",
    "En primavera empezaron a venir amigos. Primero uno, luego dos, luego una mesa llena. Nadie preguntaba por qué vivía sola. En el jardín, las conversaciones duraban hasta que se hacía de noche.",
    "Planté un limonero en el mismo sitio donde estaba el de mi abuela. No sé si dará frutos. Los árboles, como las personas, necesitan tiempo para saber si quieren quedarse.",
    "A veces, al atardecer, me siento bajo la parra y escucho el hilo de la aguja. No hay nadie cosiendo, claro. Pero la casa tiene sus ruidos, y algunos se parecen mucho a la vida.",
  ),
];

const manualDeNavegacion: DemoChapter[] = [
  chapter(
    "Mapas que no sirven para llegar",
    "Hay mapas que no sirven para llegar a ningún sitio. No porque estén mal dibujados, sino porque su propósito no es el destino: son mapas para recordar un camino, para explicar una ausencia o para sostener una promesa.",
    "El primero que recuerdo lo dibujó mi padre en el reverso de una factura. Marcaba la ruta desde nuestra casa hasta el río, con una cruz en el lugar donde se podía pescar y un círculo en el sitio donde no había que bañarse. Lo guardé durante años aunque nunca volvimos.",
    "Los mapas interiores funcionan igual. Trazamos líneas sobre lo que conocemos y dejamos en blanco lo que nos asusta. Llamamos a eso orientación, aunque muchas veces solo sea una forma elegante de evitar el mar.",
    "Navegar hacia dentro exige otra brújula. No señala el norte: señala lo que duele, lo que falta, lo que todavía no nos atrevemos a nombrar.",
    "Este libro no promete un puerto. Solo propone una manera de mirar la costa sin apartar los ojos.",
  ),
  chapter(
    "El arte de perder el rumbo",
    "Perderse no es lo mismo que extraviarse. Extraviarse implica una búsqueda; perderse, en cambio, puede ser una forma de atención. Cuando no sabemos dónde estamos, empezamos a mirar de verdad.",
    "He conocido a personas que planifican hasta el último minuto de sus viajes. Llegan a los lugares con una lista y los abandonan con otra. Nunca se pierden, pero tampoco se sorprenden.",
    "El arte de perder el rumbo consiste en aceptar que el mundo es más grande que nuestro itinerario. Consiste en doblar una esquina sin saber qué hay detrás y en escuchar una conversación ajena sin buscarle utilidad.",
    "Todo esto tiene un límite. Perderse no es una excusa para no volver. La libertad de irse sin destino necesita la valentía de regresar y contar lo que se vio.",
    "Al final, el mapa que importa no es el que nos lleva a casa, sino el que nos ayuda a reconocerla cuando llegamos.",
  ),
];

const poemasDeInvierno: DemoChapter[] = [
  chapter(
    "Primeras nieves",
    "Primera nieve\nLa nieve cae sin prisa,\ncomo si el cielo\nhubiera olvidado\nlas palabras que iba a decir.\n\nEl patio a las seis\nA las seis el patio es blanco.\nLos geranios se han dormido\ny el farol de la esquina\ncuenta historias a la nieve.\n\nHumo\nHumo de leña en los tejados.\nEl invierno se sienta\nen la silla de la cocina\ny no pide permiso.",
  ),
  chapter(
    "Ventanas",
    "Ventana del norte\nPor la ventana del norte\nentra una luz que no calienta.\nAun así, la dejo abierta:\nme gusta saber que existe.\n\nLa vecina\nLa vecina riega las plantas\naunque esté nevando.\nDice que el agua no entiende\nde estaciones.\n\nCristal\nEn el cristal de la cocina\nhay un bosque diminuto.\nLo miro antes del desayuno\ny ya no necesito salir.",
  ),
  chapter(
    "Deshielo",
    "Deshielo\nLa nieve se retira\nsin hacer ruido,\ncomo una invitada\nque no quiere despertar la casa.\n\nRío\nEl río trae barro y ramas.\nTodo lo que el invierno\nno supo decir\npasa ahora por debajo del puente.\n\nLo que queda\nQueda el agua en las botas,\nla luz en la pared,\ntu nombre en la ventana\nescrito con el dedo.",
  ),
];

const ultimaBiblioteca: DemoChapter[] = [
  chapter(
    "El archivo de los libros imposibles",
    "La biblioteca ocupaba el último piso de una torre que no aparecía en los planos de la ciudad. Se entraba por una puerta de servicio, al final de un pasillo que olía a papel mojado y a café recalentado.",
    "Nadie sabía quién la había construido. Los registros más antiguos hablaban de un archivo de libros que nunca llegaron a escribirse: novelas abandonadas en la segunda página, ensayos sin conclusión, poemas que se quedaron en el título.",
    "Lía trabajaba allí desde hacía seis meses. Su tarea consistía en clasificar las obras por el motivo de su abandono. Había cajas para el miedo, para la pereza, para la censura y para la simple falta de tiempo.",
    "La mayoría de los libros eran aburridos. Pero algunos brillaban cuando se abrían, y entonces Lía debía llamar al departamento de Incendios.",
    "—Un libro imposible no está terminado —le explicó su supervisor—. Está esperando. Es distinto.",
    "Lía asintió. Llevaba toda la tarde con una novela sin autor que empezaba así: «La primera vez que vi arder la biblioteca, supe que había encontrado mi casa».",
  ),
  chapter(
    "La bibliotecaria del turno de noche",
    "El turno de noche era el único en el que se permitía leer. Durante el día, los libros debían permanecer cerrados y los pasillos vacíos; de noche, las lámparas se encendían solas y las páginas respiraban.",
    "Lía descubrió que los libros imposibles se comunicaban entre sí. No con palabras, sino con marcas en los márgenes: una flecha, un número, una palabra subrayada dos veces. Siguiendo las marcas se podía reconstruir una historia que nunca había existido.",
    "Empezó a completar los libros. No los terminaba, porque eso habría sido una traición. Solo añadía lo que faltaba para que pudieran defenderse solos.",
    "Su supervisor lo descubrió en la tercera semana. No la despidió. Se sentó frente a ella, abrió su propio cuaderno y le mostró una novela que llevaba cuarenta años escribiendo.",
    "—No se trata de terminar —dijo—. Se trata de no dejar que se pierdan.",
    "Aquella noche, la torre entera brilló. En la ciudad, cientos de personas soñaron con libros que no recordaban haber leído.",
  ),
  chapter(
    "Todo lo que arde",
    "El incendio comenzó en la sala de Incendios, lo cual, según el supervisor, era una ironía demasiado literaria para ser casual.",
    "Lía evacuó a los libros por la escalera de servicio. Los cargaba en cajas, de diez en diez, mientras las llamas subían por el hueco del ascensor. Al llegar al último piso, el fuego ya había alcanzado la puerta.",
    "Dentro quedaba un solo libro. No tenía título ni autor. Estaba abierto por una página en blanco y, al acercarse, Lía vio que la página empezaba a llenarse con su propia letra.",
    "«No tengas miedo», decía. «Los libros que importan siempre encuentran un lector».",
    "Lía cogió el libro y corrió. Bajó los ciento veinte escalones sin mirar atrás. Cuando salió a la calle, la torre se derrumbó con un ruido suave, casi respetuoso.",
    "Al día siguiente, en el solar, alguien había dejado una caja de cartón con treinta libros. Ninguno tenía título. Todos estaban abiertos por la primera página.",
  ),
];

const libroDeLasSombras: string[][] = [
  [
    "Durante siglos, la sombra fue el primer archivo de la humanidad.",
    "Antes de la escritura, antes de la pintura, antes de cualquier",
    "sistema capaz de conservar un recuerdo, los cuerpos ya dejaban",
    "su huella sobre el suelo. La sombra decía quién estaba, dónde",
    "se movía y hacia dónde iba. Era una escritura sin papel, una",
    "memoria que dependía de la luz y del testimonio de los demás.",
    "",
    "Las primeras civilizaciones comprendieron pronto que la sombra",
    "no era una ausencia, sino una prueba. En la antigua Grecia se",
    "usaba para medir el tiempo y para orientar los templos. En las",
    "culturas andinas, las sombras de los solsticios marcaban el",
    "inicio de las cosechas. En cada caso, la sombra era un texto",
    "que solo algunos sabían leer.",
    "",
    "Este libro no pretende ser una historia exhaustiva. Su propósito",
    "es más modesto: recordar que, antes de mirar la luz, aprendimos",
    "a leer lo que la luz dejaba atrás.",
  ],
  [
    "La sombra como archivo",
    "",
    "Cuando un objeto se interpone entre la luz y una superficie, la",
    "sombra conserva su contorno. Esa es la base de la cámara oscura,",
    "del dibujo anatómico y de la fotografía. También es la base de",
    "una idea más inquietante: que toda presencia deja una marca",
    "aunque desaparezca.",
    "",
    "Los archivos modernos nacieron de esa intuición. Registrar una",
    "firma, un sello o una huella es confiar en que el paso de alguien",
    "puede conservarse más allá de su cuerpo. La sombra fue la primera",
    "tecnología de la ausencia.",
    "",
    "Pero las sombras también engañan. Cambian con la hora, con la",
    "distancia, con la calidad de la luz. Un mismo árbol puede parecer",
    "un monstruo al amanecer y una columna al mediodía. Quien lee",
    "sombras debe aprender a desconfiar de su propio instrumento.",
    "",
    "Quizá por eso las culturas antiguas asociaron la sombra con lo",
    "oculto: no porque no mostrara nada, sino porque mostraba demasiado.",
  ],
  [
    "Sombras que nos preceden",
    "",
    "Hay sombras que no pertenecen a nadie. Las proyectan las nubes,",
    "los edificios, los aviones que ya no están. Nuestras ciudades",
    "están llenas de esas presencias sin dueño, y sin embargo las",
    "leemos como si tuvieran un significado.",
    "",
    "La historia de la sombra es también la historia de la luz",
    "artificial. Cuando aprendimos a iluminar la noche, dejamos de",
    "depender del sol para leer el mundo. Ganamos intimidad y perdimos",
    "una forma de orientación colectiva.",
    "",
    "Hoy las sombras se han vuelto un residuo. Las eliminamos con",
    "lámparas, con pantallas, con luces que no proyectan. Pero siguen",
    "ahí, debajo de los objetos, esperando que alguien baje la mirada.",
    "",
    "Este libro termina sin una conclusión, porque las sombras no",
    "concluyen: se alargan. Lo único que podemos hacer es aprender a",
    "caminar dentro de ellas sin perder del todo la luz.",
  ],
];

const ciudadYFantasmas: string[][] = [
  [
    "Toda ciudad tiene una memoria que no aparece en los mapas.",
    "Está hecha de voces, de olores, de esquinas que cambiaron de",
    "nombre. Caminamos por ella sin saber que la llevamos encima.",
    "",
    "Los fantasmas de una ciudad no son aparecidos. Son costumbres",
    "que sobreviven a quienes las inventaron: la panadería que abre",
    "a las cinco, el banco donde siempre se sientan los mismos",
    "jubilados, la canción que alguien silba sin recordar de dónde",
    "la sacó.",
    "",
    "Este libro es un intento de cartografiar esa ciudad invisible.",
    "No busca lo extraordinario, sino lo que se repite hasta volverse",
    "invisible. Porque una ciudad no se conoce por sus monumentos,",
    "sino por lo que sus habitantes hacen sin pensar.",
    "",
    "La primera vez que me di cuenta de esto fue en un autobús",
    "nocturno. El conductor saludaba a cada pasajero por su nombre.",
    "Ninguno de ellos parecía sorprendido. Yo era el único que",
    "acababa de llegar.",
  ],
  [
    "Los fantasmas no habitan las casas",
    "",
    "Habitán los huecos entre las casas: los patios compartidos, las",
    "terrazas que se miran, los tendederos que cruzan una calle de",
    "lado a lado. Allí donde la intimidad se vuelve pública sin",
    "dejar de ser íntima.",
    "",
    "En el barrio de San Martín hay una mujer que pone la radio a la",
    "misma hora desde hace cuarenta años. Nadie sabe su nombre. Si",
    "un día no la pone, los vecinos llaman a la puerta. Esa es su",
    "manera de existir: una costumbre compartida.",
    "",
    "Los fantasmas también pueden ser objetos. Una cabina telefónica",
    "sin teléfono, un cine convertido en supermercado, un cartel que",
    "anuncia una obra que ya no se representa. El paisaje urbano",
    "está lleno de ruinas que no hemos decidido derribar.",
    "",
    "Conservarlas no es nostalgia. Es una forma de recordar que la",
    "ciudad no siempre fue así, y que por tanto puede dejar de serlo.",
  ],
  [
    "Hay una plaza que solo existe de noche",
    "",
    "De día es un aparcamiento. De noche, cuando se vacía, aparece",
    "la plaza que fue: bancos de piedra, una fuente sin agua, dos",
    "árboles que se tocan por arriba. Los vecinos mayores la llaman",
    "«la plaza de antes», como si el tiempo fuera una dirección.",
    "",
    "En verano, algunos sacan sillas a la puerta. No hay programa",
    "ni asociación. Simplemente se sientan. A veces hablan; a veces",
    "miran pasar los coches. La plaza recupera su función sin que",
    "nadie lo haya decidido.",
    "",
    "Esa es la clase de fantasma que me interesa: el que no pide",
    "ser recordado, solo un poco de espacio. No aparece en las guías",
    "turísticas ni en los presupuestos municipales. Pero si desaparece,",
    "la ciudad pierde una habitación.",
    "",
    "Y las ciudades, como las casas, se conocen por las habitaciones",
    "que no se enseñan.",
  ],
  [
    "Al final, la ciudad nos recuerda",
    "",
    "Cuando volvemos después de años, no somos nosotros quienes",
    "reconocen la ciudad: es ella la que nos reconoce. El cuerpo",
    "recuerda el camino a la panadería, la altura del escalón, el",
    "frío de la barandilla. La memoria no está en la cabeza.",
    "",
    "Los fantasmas que he descrito no son tristes. Son la prueba de",
    "que algo ocurrió y dejó una forma. Una ciudad sin fantasmas",
    "sería una ciudad sin historia, condenada a inaugurar su propia",
    "vida cada mañana.",
    "",
    "Quizá por eso escribimos sobre ellas. No para recuperar lo",
    "perdido, sino para aprender a vivir con lo que queda.",
    "",
    "Este libro termina aquí, en una plaza sin nombre, con una silla",
    "vacía y una luz encendida en una ventana. Alguien mira. Alguien",
    "está a punto de llegar.",
  ],
];

export const demoBookSeeds: DemoBookSeed[] = [
  {
    id: "book-mareas",
    title: "El libro de las mareas",
    author: "Marina Soler",
    format: "epub",
    description:
      "Una mujer vuelve al pueblo costero donde desapareció su madre y descubre que el mar guarda una biblioteca de nombres perdidos.",
    publisher: "Ediciones Alba",
    language: "es",
    sizeBytes: 2_840_112,
    addedAt: "2026-10-02T18:40:00Z",
    lastReadAt: "2026-11-22T09:12:00Z",
    progress: 0.4,
    locator: { section: 1, offset: 0 },
    favorite: true,
    readOnly: false,
    categoryIds: ["cat-novela", "cat-club"],
    palette: ["#0b2239", "#2f6273", "#f4e9d4"],
    motif: "waves",
    settings: {
      mode: "paged",
      fontFace: "newsreader",
      columnWidth: 760,
      textAlign: "justify",
    },
    chapters: mareas,
  },
  {
    id: "book-dias-de-sal",
    title: "Los días de sal",
    author: "Tomás Iriarte",
    format: "epub",
    description:
      "Un verano de redes vacías, cartas que no llegan y una familia que aprende a hablar sin decir la verdad.",
    publisher: "Libros del Muelle",
    language: "es",
    sizeBytes: 1_902_388,
    addedAt: "2026-10-18T10:15:00Z",
    lastReadAt: "2026-11-21T21:38:00Z",
    progress: 0.12,
    locator: { section: 0, offset: 0 },
    favorite: false,
    readOnly: false,
    categoryIds: ["cat-novela"],
    palette: ["#4a3b2a", "#b07d4f", "#f7ecd8"],
    motif: "lines",
    settings: { mode: "scroll", fontFace: "literata", margin: 56 },
    chapters: diasDeSal,
  },
  {
    id: "book-cartas-faro",
    title: "Cartas desde el faro",
    author: "Lucía Ferrer",
    format: "epub",
    description:
      "Siete años de cartas no enviadas, un faro que sigue encendido y una mujer que decide quedarse.",
    publisher: "Norte Sur",
    language: "es",
    sizeBytes: 1_244_900,
    addedAt: "2026-09-01T08:05:00Z",
    lastReadAt: "2026-10-29T23:10:00Z",
    progress: 1,
    locator: { section: 2, offset: 0 },
    favorite: false,
    readOnly: false,
    categoryIds: ["cat-novela", "cat-club"],
    palette: ["#18243b", "#7d8ea8", "#f0e6d2"],
    motif: "moon",
    chapters: cartasDelFaro,
  },
  {
    id: "book-mecanica",
    title: "La mecánica del silencio",
    author: "Andrés Vela",
    format: "epub",
    description:
      "Un relojero hereda una máquina capaz de escuchar el pasado y descubre que la memoria también se puede reparar.",
    publisher: "Páginas de Varena",
    language: "es",
    sizeBytes: 2_112_760,
    addedAt: "2026-11-06T19:22:00Z",
    lastReadAt: null,
    progress: 0,
    locator: { section: 0, offset: 0 },
    favorite: false,
    readOnly: false,
    categoryIds: ["cat-novela", "cat-ciencia"],
    palette: ["#2b2a33", "#8a7357", "#f2e8d5"],
    motif: "arch",
    chapters: mecanicaDelSilencio,
  },
  {
    id: "book-sombras-pdf",
    title: "Breve historia de las sombras",
    author: "Clara Nogueira",
    format: "pdf",
    description:
      "Un ensayo sobre la sombra como primera tecnología de la memoria, desde la cámara oscura hasta las ciudades iluminadas.",
    publisher: "Cuadernos de la Luz",
    language: "es",
    sizeBytes: 4_602_310,
    addedAt: "2026-08-14T12:00:00Z",
    lastReadAt: "2026-11-17T18:45:00Z",
    progress: 0.28,
    locator: { section: 0, offset: 0 },
    favorite: false,
    readOnly: false,
    categoryIds: ["cat-ensayo", "cat-historia"],
    palette: ["#151515", "#4a4a4a", "#e8dcc8"],
    motif: "grid",
    pdfPages: libroDeLasSombras,
  },
  {
    id: "book-jardin",
    title: "El jardín de las horas lentas",
    author: "Irene Salvatierra",
    format: "epub",
    description:
      "Una nieta hereda la casa de su abuela, un cuaderno de siestas y un jardín que necesita más tiempo que cuidados.",
    publisher: "Buganvilla",
    language: "es",
    sizeBytes: 1_678_220,
    addedAt: "2026-09-22T16:30:00Z",
    lastReadAt: "2026-11-19T20:02:00Z",
    progress: 0.67,
    locator: { section: 1, offset: 0 },
    favorite: true,
    readOnly: false,
    categoryIds: ["cat-novela", "cat-club"],
    palette: ["#314a3b", "#8aa27a", "#f6ead2"],
    motif: "leaves",
    settings: { theme: "sepia", fontFace: "garamond", mode: "paged" },
    chapters: jardinDeLasHoras,
  },
  {
    id: "book-ciudad-pdf",
    title: "La ciudad y sus fantasmas",
    author: "Mateo Rivera",
    format: "pdf",
    description:
      "Crónicas de una ciudad invisible: plazas que solo existen de noche, costumbres que sobreviven a sus dueños.",
    publisher: "Mapa Común",
    language: "es",
    sizeBytes: 3_920_440,
    addedAt: "2026-11-11T09:10:00Z",
    lastReadAt: null,
    progress: 0,
    locator: { section: 0, offset: 0 },
    favorite: false,
    readOnly: false,
    categoryIds: ["cat-ensayo", "cat-historia"],
    palette: ["#292524", "#7c6a58", "#f3e6d0"],
    motif: "dots",
    pdfPages: ciudadYFantasmas,
  },
  {
    id: "book-manual",
    title: "Manual de navegación interior",
    author: "Julia Arce",
    format: "epub",
    description:
      "Un ensayo breve sobre mapas que no sirven para llegar y sobre la valentía de perder el rumbo.",
    publisher: "Brújula",
    language: "es",
    sizeBytes: 886_310,
    addedAt: "2026-07-03T11:45:00Z",
    lastReadAt: "2026-11-10T08:20:00Z",
    progress: 0.5,
    locator: { section: 0, offset: 0 },
    favorite: false,
    readOnly: true,
    categoryIds: ["cat-ensayo"],
    palette: ["#1e3a4c", "#6d99a8", "#f5ecd9"],
    motif: "waves",
    chapters: manualDeNavegacion,
  },
  {
    id: "book-poemas",
    title: "Poemas de invierno",
    author: "Nora Vidal",
    format: "epub",
    description:
      "Tres cuadernos de nieve, ventanas y deshielo. Poesía breve para leer despacio, con las manos frías.",
    publisher: "Cuadernos del Frío",
    language: "es",
    sizeBytes: 640_180,
    addedAt: "2026-06-18T17:00:00Z",
    lastReadAt: "2026-11-02T22:15:00Z",
    progress: 1,
    locator: { section: 2, offset: 0 },
    favorite: true,
    readOnly: false,
    categoryIds: ["cat-poesia"],
    palette: ["#26354a", "#9aa9bd", "#f7f0e3"],
    motif: "lines",
    settings: { mode: "scroll", fontFace: "literata", textAlign: "left" },
    chapters: poemasDeInvierno,
  },
  {
    id: "book-ultima-biblioteca",
    title: "La última biblioteca",
    author: "Ada Torres",
    format: "epub",
    description:
      "En una torre que no aparece en los planos, una bibliotecaria clasifica libros que nunca llegaron a escribirse.",
    publisher: "Futuro Próximo",
    language: "es",
    sizeBytes: 2_402_880,
    addedAt: "2026-11-20T13:30:00Z",
    lastReadAt: null,
    progress: 0,
    locator: { section: 0, offset: 0 },
    favorite: false,
    readOnly: false,
    categoryIds: ["cat-ciencia", "cat-novela"],
    palette: ["#1c1b2e", "#5b4b8a", "#f4e8ce"],
    motif: "sun",
    audiobook: true,
    chapters: ultimaBiblioteca,
  },
];

export const demoTrashSeeds: DemoBookSeed[] = [
  {
    id: "book-cuaderno-rutas",
    title: "Cuaderno de rutas",
    author: "Elena Márquez",
    format: "epub",
    description:
      "Notas de viaje y pequeños mapas dibujados a mano durante un año de caminatas por la costa.",
    publisher: "Edición personal",
    language: "es",
    sizeBytes: 512_400,
    addedAt: "2026-05-10T10:00:00Z",
    lastReadAt: "2026-10-01T18:00:00Z",
    progress: 0.34,
    locator: { section: 0, offset: 0 },
    favorite: false,
    readOnly: false,
    categoryIds: ["cat-ensayo"],
    palette: ["#4a3426", "#a67c52", "#f6ead7"],
    motif: "arch",
  },
];

export interface DemoAnnotationSeed {
  id: string;
  bookId: string;
  chapterIndex: number;
  quote: string;
  kind: "highlight" | "underline" | "bookmark" | "strike";
  color: string;
  note: string | null;
}

export const demoAnnotationSeeds: DemoAnnotationSeed[] = [
  {
    id: "marker-demo-1",
    bookId: "book-mareas",
    chapterIndex: 0,
    quote: "El mar nunca olvida a quienes lo escuchan",
    kind: "highlight",
    color: "#ffd166",
    note: "La advertencia del abuelo vuelve al final del capítulo.",
  },
  {
    id: "marker-demo-2",
    bookId: "book-mareas",
    chapterIndex: 1,
    quote: "La llave estaba fría, más fría que el agua",
    kind: "underline",
    color: "#8ecae6",
    note: null,
  },
  {
    id: "marker-demo-3",
    bookId: "book-dias-de-sal",
    chapterIndex: 0,
    quote: "el primer día del silencio que duraría tres meses",
    kind: "highlight",
    color: "#ffb4a2",
    note: null,
  },
  {
    id: "marker-demo-4",
    bookId: "book-jardin",
    chapterIndex: 1,
    quote: "El tiempo de las siestas es el único que no se pierde",
    kind: "highlight",
    color: "#b7e4c7",
    note: "Lo copio en mi cuaderno.",
  },
];

export interface DemoNoteSeed {
  id: string;
  bookId: string;
  chapterIndex: number | null;
  title: string;
  body: string;
  text: string | null;
}

export const demoNoteSeeds: DemoNoteSeed[] = [
  {
    id: "note-demo-1",
    bookId: "book-mareas",
    chapterIndex: 1,
    title: "La puerta y la marea",
    body:
      "La puerta solo aparece cuando baja la marea, igual que los recuerdos familiares: hay que esperar el momento exacto para que se abran. La llave podría ser un símbolo de la responsabilidad de mirar lo que otros callaron.",
    text: "La llave y la puerta",
  },
  {
    id: "note-demo-2",
    bookId: "book-jardin",
    chapterIndex: 2,
    title: "La silla de más",
    body:
      "Mi abuela también dejaba una silla vacía en la cocina. Nunca supe si era por costumbre o por esperanza.",
    text: null,
  },
];

export interface DemoExtractSeed {
  id: string;
  bookId: string;
  chapterIndex: number;
  quote: string;
  note: string;
}

export const demoExtractSeeds: DemoExtractSeed[] = [
  {
    id: "extract-demo-1",
    bookId: "book-mareas",
    chapterIndex: 0,
    quote: "las historias viajaban con la marea",
    note: "Una imagen que resume toda la novela.",
  },
  {
    id: "extract-demo-2",
    bookId: "book-ultima-biblioteca",
    chapterIndex: 0,
    quote: "Un libro imposible no está terminado",
    note: "Para la club de lectura.",
  },
];

export function findBookSeed(bookId: string): DemoBookSeed | undefined {
  return (
    [...demoBookSeeds, ...demoTrashSeeds].find((book) => book.id === bookId) ??
    dynamicSeeds.get(bookId)
  );
}

const dynamicSeeds = new Map<string, DemoBookSeed>();

/** Registra libros creados en tiempo de ejecución (tejidos, importaciones). */
export function registerBookSeed(seed: DemoBookSeed): void {
  dynamicSeeds.set(seed.id, seed);
}

export function chaptersFor(seed: DemoBookSeed): DemoChapter[] {
  if (seed.chapters) return seed.chapters;
  // Los libros de la papelera y cualquier importación nueva reciben un
  // contenido de muestra coherente para que el lector nunca quede vacío.
  return [
    chapter(
      seed.title,
      `Este cuaderno pertenece a ${seed.author}. Las páginas están numeradas a lápiz y muchas esquinas aparecen dobladas, como si alguien hubiera querido volver a un lugar sin recordar cuál.`,
      "La primera ruta empieza en la plaza del pueblo y termina en una cala sin nombre. Hay una nota en el margen: «No seguir el sendero marcado. El sendero está hecho para los que ya saben adónde van».",
      "A mitad del cuaderno aparece un mapa dibujado sobre una servilleta. Las distancias no coinciden con ningún mapa oficial, pero el dibujo tiene una precisión extraña, como si hubiera sido copiado de la memoria.",
      "En la última página hay una lista de cosas que no deben olvidarse: el olor del romero, la hora en que se enciende el faro, el nombre de la mujer que vendía higos en el cruce.",
      "El cuaderno termina sin firma. Solo una frase subrayada dos veces: «Volver es una forma de seguir».",
    ),
  ];
}

function escapeXml(value: string): string {
  return value
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&apos;");
}

function wrapTitle(title: string, max = 16): string[] {
  const words = title.split(/\s+/);
  const lines: string[] = [];
  let current = "";
  for (const word of words) {
    const next = current ? `${current} ${word}` : word;
    if (next.length > max && current) {
      lines.push(current);
      current = word;
    } else {
      current = next;
    }
  }
  if (current) lines.push(current);
  return lines.slice(0, 4);
}

function motifSvg(motif: CoverMotif, accent: string): string {
  switch (motif) {
    case "waves":
      return `<g fill="none" stroke="${accent}" stroke-width="2" opacity="0.5">
        <path d="M-20 610 C90 570 170 650 280 610 S470 570 580 610 S770 650 900 610"/>
        <path d="M-20 650 C90 610 170 690 280 650 S470 610 580 650 S770 690 900 650"/>
        <path d="M-20 690 C90 650 170 730 280 690 S470 650 580 690 S770 730 900 690"/>
      </g>`;
    case "sun":
      return `<circle cx="620" cy="210" r="108" fill="${accent}" opacity="0.22"/>
        <circle cx="620" cy="210" r="74" fill="none" stroke="${accent}" stroke-width="2" opacity="0.55"/>
        <circle cx="620" cy="210" r="42" fill="${accent}" opacity="0.16"/>`;
    case "arch":
      return `<path d="M300 760 L300 500 Q450 330 600 500 L600 760" fill="none" stroke="${accent}" stroke-width="3" opacity="0.45"/>
        <path d="M350 760 L350 520 Q450 390 550 520 L550 760" fill="none" stroke="${accent}" stroke-width="1.5" opacity="0.32"/>`;
    case "leaves":
      return `<g fill="${accent}" opacity="0.3">
        <ellipse cx="610" cy="610" rx="38" ry="120" transform="rotate(-35 610 610)"/>
        <ellipse cx="680" cy="640" rx="30" ry="96" transform="rotate(20 680 640)"/>
        <ellipse cx="550" cy="670" rx="26" ry="80" transform="rotate(-60 550 670)"/>
      </g>`;
    case "grid":
      return `<g stroke="${accent}" stroke-width="1" opacity="0.25">
        <path d="M0 180 H900 M0 300 H900 M0 420 H900 M0 540 H900 M0 660 H900"/>
        <path d="M120 0 V760 M240 0 V760 M360 0 V760 M480 0 V760 M600 0 V760 M720 0 V760 M840 0 V760"/>
      </g>`;
    case "dots":
      return `<g fill="${accent}" opacity="0.28">
        ${Array.from({ length: 36 }, (_, index) => {
          const x = 90 + (index % 9) * 90;
          const y = 470 + Math.floor(index / 9) * 74;
          return `<circle cx="${x}" cy="${y}" r="${3 + (index % 4)}"/>`;
        }).join("")}
      </g>`;
    case "moon":
      return `<circle cx="640" cy="220" r="96" fill="${accent}" opacity="0.25"/>
        <circle cx="676" cy="196" r="88" fill="${accent}" opacity="0.4"/>`;
    case "lines":
      return `<g stroke="${accent}" stroke-width="2" opacity="0.3">
        <path d="M120 760 V420 M180 760 V360 M240 760 V470 M300 760 V320 M360 760 V500"/>
      </g>`;
  }
}

export function coverDataUri(seed: DemoBookSeed): string {
  const [from, to, ink] = seed.palette;
  const lines = wrapTitle(seed.title);
  const startY = lines.length > 2 ? 360 : 400;
  const title = lines
    .map(
      (line, index) =>
        `<text x="72" y="${startY + index * 58}" font-family="Georgia, 'Times New Roman', serif" font-size="${lines.length > 2 ? 48 : 54}" fill="${ink}">${escapeXml(line)}</text>`,
    )
    .join("");
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="600" height="900" viewBox="0 0 600 900">
    <defs>
      <linearGradient id="bg" x1="0" y1="0" x2="0.7" y2="1">
        <stop offset="0" stop-color="${from}"/>
        <stop offset="1" stop-color="${to}"/>
      </linearGradient>
    </defs>
    <rect width="600" height="900" fill="url(#bg)"/>
    <rect x="24" y="24" width="552" height="852" fill="none" stroke="${ink}" stroke-width="1" opacity="0.4"/>
    ${motifSvg(seed.motif, ink)}
    ${title}
    <text x="72" y="${startY + lines.length * 58 + 38}" font-family="'Helvetica Neue', Arial, sans-serif" font-size="22" letter-spacing="2" fill="${ink}" opacity="0.86">${escapeXml(seed.author.toUpperCase())}</text>
    <text x="72" y="812" font-family="'Helvetica Neue', Arial, sans-serif" font-size="14" letter-spacing="3" fill="${ink}" opacity="0.55">${escapeXml(seed.publisher.toUpperCase())}</text>
    <circle cx="510" cy="800" r="9" fill="none" stroke="${ink}" stroke-width="1.5" opacity="0.6"/>
  </svg>`;
  return `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`;
}

export function demoBookSummary(seed: DemoBookSeed): BookSummary {
  return {
    id: seed.id,
    title: seed.title,
    author: seed.author,
    format: seed.format,
    coverPath: coverDataUri(seed),
    sizeBytes: seed.sizeBytes,
    addedAt: seed.addedAt,
    language: seed.language,
    publisher: seed.publisher,
    description: seed.description,
    lastReadAt: seed.lastReadAt,
    progress: seed.progress,
    origin: "imported",
    favorite: seed.favorite,
    readOnly: seed.readOnly,
    receivedShareId: seed.readOnly ? "share-demo-club" : null,
    categoryIds: [...seed.categoryIds],
  };
}

export const demoImportedSeeds: DemoBookSeed[] = [
  {
    id: "book-isla-ecos",
    title: "La isla de los ecos",
    author: "Sara Belmonte",
    format: "epub",
    description:
      "Una isla donde cada palabra pronunciada vuelve con la voz de otra persona. Novela breve de misterio y memoria.",
    publisher: "Marea Alta",
    language: "es",
    sizeBytes: 1_344_200,
    addedAt: new Date().toISOString(),
    lastReadAt: null,
    progress: 0,
    locator: { section: 0, offset: 0 },
    favorite: false,
    readOnly: false,
    categoryIds: ["cat-novela"],
    palette: ["#17324a", "#4f7f8c", "#f4ebd9"],
    motif: "dots",
    chapters: [
      chapter(
        "La isla de los ecos",
        "La isla aparecía en los mapas con un nombre distinto cada década. Los pescadores decían que era porque nadie se ponía de acuerdo, pero la verdad era más sencilla: la isla cambiaba de nombre para que no pudieran encontrarla dos veces.",
        "Cuando Vera desembarcó, lo primero que oyó fue su propia voz diciendo «buenos días» desde el otro lado del muelle. Se giró. No había nadie.",
        "El farero la esperaba con una taza de café y una libreta. —No hable alto —le advirtió—. Aquí las palabras tardan en volver, pero cuando vuelven, ya no son suyas.",
        "Vera abrió la libreta. En la primera página había una lista de nombres tachados. El suyo estaba al final, escrito con una letra que reconocía demasiado bien.",
        "Esa noche, la isla repitió su nombre hasta que el mar se quedó quieto.",
      ),
    ],
  },
];
