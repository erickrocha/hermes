package com.erocha.hermes.client.driver

import com.erocha.hermes.client.shared.FuelReceiptParser
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class FuelReceiptParserTest {
    private val clean = """
        POSTO BOM CAMINHO COMBUSTIVEIS LTDA
        CNPJ: 12.345.678/0001-90
        Rua das Flores, 100 - Limeira/SP
        DOCUMENTO AUXILIAR DA NOTA FISCAL DE CONSUMIDOR ELETRÔNICA
        Código Descrição Qtd Un Vl Unit Vl Total
        001 DIESEL S10 45,230 L 5,899 266,81
        Qtde. total de itens 1
        Valor total R$ 266,81
        Forma de pagamento Cartão de Crédito 266,81
        NFC-e nº 12345 Série 1 Emissão 27/10/2026 14:32:10
    """.trimIndent()

    @Test
    fun aCleanReceiptYieldsEverythingTheFormNeeds() {
        val scanned = FuelReceiptParser.parse(clean)
        assertEquals(45.23, scanned.volumeLiters!!, 0.0001)
        assertEquals(26681L, scanned.valueCents)
        assertEquals("POSTO BOM CAMINHO COMBUSTIVEIS LTDA", scanned.station)
        assertEquals("2026-10-27T14:32:10", scanned.dateTime)
        assertEquals("12.345.678/0001-90", scanned.cnpj)
    }

    @Test
    fun anItemSplitInTwoLinesByTheRecognizerIsStillRead() {
        val scanned = FuelReceiptParser.parse(
            """
            AUTO POSTO SOL
            001 GASOLINA COMUM
            38,500 L 6,299 242,51
            VALOR TOTAL RS
            242,51
            Emissão 05/03/2027 08:10
            """.trimIndent(),
        )
        assertEquals(38.5, scanned.volumeLiters!!, 0.0001)
        assertEquals(24251L, scanned.valueCents)
        assertEquals("2027-03-05T08:10:00", scanned.dateTime)
    }

    @Test
    fun thousandsSeparatorsInTheTotalAreUnderstood() {
        val scanned = FuelReceiptParser.parse("POSTO GRANDE\nDIESEL S10 200,000 L 5,900 1.180,00\nTOTAL R$ 1.180,00")
        assertEquals(200.0, scanned.volumeLiters!!, 0.0001)
        assertEquals(118000L, scanned.valueCents)
    }

    @Test
    fun aQuantityThatDoesNotMatchPriceAndTotalIsNotTrustedAsTheItem() {
        // 10 x 5,00 = 50,00, not 80,00: the item is discarded and the liters come from the unit instead.
        val scanned = FuelReceiptParser.parse("POSTO X\nDIESEL 10,000 5,000 80,00\n7,5 L\nTOTAL R$ 80,00")
        assertEquals(7.5, scanned.volumeLiters!!, 0.0001)
        assertEquals(8000L, scanned.valueCents)
    }

    @Test
    fun arlaIsNotTakenForTheFuel() {
        val scanned = FuelReceiptParser.parse("POSTO Y\nARLA 32 20,000 L 4,000 80,00\nDIESEL S10 50,000 L 5,000 250,00\nVALOR TOTAL R$ 330,00")
        assertEquals(50.0, scanned.volumeLiters!!, 0.0001)
        assertEquals(33000L, scanned.valueCents)
    }

    @Test
    fun textWithNoAmountsReadsNothingAndNeverGuessesAnImpossibleDate() {
        val scanned = FuelReceiptParser.parse("foto borrada\n99/99/2026 25:99")
        assertNull(scanned.volumeLiters)
        assertNull(scanned.valueCents)
        assertNull(scanned.dateTime)
        assertTrue(FuelReceiptParser.parse("   \n ").isEmpty)
    }

    @Test
    fun anAmountSplitByASpaceAfterTheCommaIsStillAnAmount() {
        val scanned = FuelReceiptParser.parse("POSTO Z\nValor total R$ 266, 81")
        assertEquals(26681L, scanned.valueCents)
    }
}
