// Valores fijos de la app. Los datos descriptivos de cada evento (imagen,
// categoría, dirección, texto y coordenadas) viven en Supabase; los del
// catálogo de ejemplo están en supabase/semilla.

export const CATEGORIAS = ['Todos', 'Eventos', 'Conciertos', 'Teatro']

// Cuenta de ejemplo para probar transferencias.
export const CUENTA_EJEMPLO = 'GA34IJVLXRTUB2B4WKVDCWEO44UNUTBXVPMCK6UWIKJZL2PEQXBCSTHL'

// Portada genérica para eventos sin imagen (por ejemplo, los que se crean desde la app).
export const portadaGenerica = (titulo) => ({
  fondo: 'linear-gradient(135deg, #3a3a3c 0%, #1c1c1e 100%)',
  texto: titulo.toUpperCase(),
  estilo: 'nuevo',
})
